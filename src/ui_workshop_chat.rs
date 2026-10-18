use super::*;
use crate::local_assistant::{self, Proposal, Status};

#[derive(Default)]
pub(super) struct WorkshopChat {
    pub open: bool,
    pub input: String,
    pub messages: VecDeque<String>,
    pub pending: Option<ChatPreview>,
}

pub(super) struct ChatPreview {
    pub proposal: Proposal,
    pub scope: (usize, String, usize),
    pub original: Option<Vec<Step>>,
    pub request: String,
}

impl WorkshopChat {
    fn say(&mut self, text: impl Into<String>) {
        self.messages
            .push_back(text.into().chars().take(240).collect());
        while self.messages.len() > 30 {
            self.messages.pop_front();
        }
    }
}

fn sensitive(step: &Step) -> bool {
    matches!(
        step.active(),
        Step::AutoHotkey { .. }
            | Step::HttpRequest { .. }
            | Step::UdpSend { .. }
            | Step::CloseWindow
            | Step::WindowOp { .. }
            | Step::Launch { .. }
    )
}

impl App {
    pub(super) fn workshop_chat_send(&mut self, text: String, selection: Option<&[usize]>) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let request = text.chars().take(400).collect::<String>();
        self.workshop_chat.say(format!("Vous : {request}"));
        self.workshop_chat.pending = None;
        let scope = (self.config.active, self.config.page.clone(), self.selected);
        let mode = self.preferences.workshop_chat_mode.as_str();
        if mode == "ia" {
            self.workshop_chat.say("IA : utilise le connecteur MCP DeckForge pour proposer des modifications ; aucune requête IA directe n'est envoyée.");
            self.assistant_settings = true;
            return;
        }
        let normalized = request.to_lowercase();
        if normalized == "expliquer ce bloc" {
            let step = selection.and_then(|path| {
                crate::workshop_ops::get(&mut self.config.keys()[self.selected].steps.clone(), path)
                    .cloned()
            });
            if let Some(step) = step {
                let (detail, missing) = local_assistant::explain(&step);
                self.workshop_chat.say(detail);
                for field in missing {
                    self.workshop_chat.say(field.reason);
                }
            } else {
                self.workshop_chat.say("Sélectionne un bloc à expliquer.");
            }
            return;
        }
        if normalized == "corriger cette macro" || normalized == "optimiser" {
            let before = self.config.keys()[self.selected].steps.clone();
            let suggestions = local_assistant::diagnose(&before);
            if let Some(fix) = suggestions.into_iter().find(|s| s.steps != before) {
                self.workshop_chat.say(fix.reason);
                self.workshop_chat.pending = Some(ChatPreview {
                    proposal: Proposal {
                        steps: fix.steps,
                        notes: vec![],
                        missing: vec![],
                        status: Status::Reconnue,
                    },
                    scope,
                    original: Some(before),
                    request,
                });
            } else {
                self.workshop_chat.say(
                    "Aucune correction locale sûre à proposer. Décris la modification souhaitée.",
                );
            }
            return;
        }
        if normalized == "créer une macro" {
            self.workshop_chat.say(
                "Décris les actions souhaitées. Exemple : attends 2 s, puis passe sur Gaming.",
            );
            return;
        }
        let request = if normalized == "ajouter une temporisation" {
            "attends 1 s".to_owned()
        } else {
            request
        };
        let proposal = local_assistant::propose(&request, &self.config);
        match &proposal.status {
            Status::Inconnue => self.workshop_chat.say("Demande inconnue en local. Exemples : attends 2 s ; ouvre https://exemple.fr ; tape bonjour."),
            Status::Ambigue { question, .. } => self.workshop_chat.say(question.clone()),
            Status::Reconnue => self.workshop_chat.say(format!("Aperçu de {} bloc(s), sans exécution.", proposal.steps.len())),
        }
        self.workshop_chat.pending = Some(ChatPreview {
            proposal,
            scope,
            original: None,
            request,
        });
    }

    pub(super) fn workshop_chat(&mut self, ctx: &egui::Context, selection: Option<&[usize]>) {
        let scope = (self.config.active, self.config.page.clone(), self.selected);
        let stale = self
            .workshop_chat
            .pending
            .as_ref()
            .is_some_and(|p| p.scope != scope);
        if !self.workshop_chat.open {
            return;
        }
        let mut open = self.workshop_chat.open;
        let mut send = None;
        let mut choice = None;
        let mut action = None;
        let mut save_mode = false;
        let screen = ctx.screen_rect();
        egui::Window::new("Assistant · Atelier")
            .id(egui::Id::new("workshop_chat_window"))
            .open(&mut open)
            .default_pos(egui::pos2(
                (screen.right() - 356.0).max(8.0),
                (screen.bottom() - 495.0).max(32.0),
            ))
            .default_width(340.0)
            .min_width(280.0)
            .max_width(360.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.set_max_height((screen.height() - 110.0).max(220.0));
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(match self.preferences.workshop_chat_mode.as_str() {
                            "ia" if self.assistant.is_some() => "IA",
                            "ia" => "Indisponible",
                            _ => "Local",
                        })
                        .color(theme::palette(ctx).muted),
                    );
                    egui::ComboBox::from_id_salt("workshop_chat_mode")
                        .selected_text(match self.preferences.workshop_chat_mode.as_str() {
                            "local" => "Local",
                            "ia" => "IA",
                            _ => "Automatique",
                        })
                        .show_ui(ui, |ui| {
                            for (value, label) in
                                [("local", "Local"), ("auto", "Automatique"), ("ia", "IA")]
                            {
                                save_mode |= ui
                                    .selectable_value(
                                        &mut self.preferences.workshop_chat_mode,
                                        value.into(),
                                        label,
                                    )
                                    .changed();
                            }
                        });
                });
                egui::ScrollArea::vertical()
                    .id_salt("chat_history")
                    .max_height((screen.height() * 0.22).min(150.0))
                    .show(ui, |ui| {
                        for message in &self.workshop_chat.messages {
                            ui.label(message);
                        }
                    });
                if let Some(pending) = &self.workshop_chat.pending {
                    ui.separator();
                    if stale {
                        ui.colored_label(
                            theme::palette(ctx).muted,
                            "Proposition périmée : touche, page ou profil changé.",
                        );
                    }
                    match &pending.proposal.status {
                        Status::Ambigue { question, choix } => {
                            ui.label(question);
                            for option in choix {
                                if ui.button(option).clicked() {
                                    choice = Some(option.clone());
                                }
                            }
                        }
                        Status::Reconnue => {
                            egui::ScrollArea::vertical()
                                .id_salt("chat_preview")
                                .max_height((screen.height() * 0.24).min(175.0))
                                .show(ui, |ui| {
                                    if let Some(before) = &pending.original {
                                        ui.label(format!("Avant : {} bloc(s)", before.len()));
                                        for step in before {
                                            ui.label(format!(
                                                "- {}",
                                                super::extras::block_summary(step)
                                            ));
                                        }
                                        ui.label(format!(
                                            "Après : {} bloc(s)",
                                            pending.proposal.steps.len()
                                        ));
                                    }
                                    for (i, step) in pending.proposal.steps.iter().enumerate() {
                                        ui.label(format!(
                                            "{}  {}",
                                            step.name(),
                                            super::extras::block_summary(step)
                                        ));
                                        for missing in pending
                                            .proposal
                                            .missing
                                            .iter()
                                            .filter(|m| m.path.first() == Some(&i))
                                        {
                                            ui.colored_label(
                                                Color32::LIGHT_RED,
                                                format!("À compléter : {}", missing.field),
                                            );
                                        }
                                        if sensitive(step) {
                                            ui.colored_label(
                                                Color32::LIGHT_RED,
                                                "Action sensible : vérifier avant insertion.",
                                            );
                                        }
                                    }
                                    for note in &pending.proposal.notes {
                                        ui.label(note);
                                    }
                                });
                            ui.horizontal(|ui| {
                                let valid = !stale
                                    && pending.proposal.insertable()
                                    && (pending.original.is_some()
                                        || self.config.keys()[self.selected].steps.len()
                                            + pending.proposal.steps.len()
                                            <= 256);
                                if ui
                                    .add_enabled(
                                        valid,
                                        egui::Button::new(if pending.original.is_some() {
                                            "Appliquer"
                                        } else {
                                            "Ajouter"
                                        }),
                                    )
                                    .clicked()
                                {
                                    action = Some(0);
                                }
                                if ui.button("Ajuster").clicked() {
                                    action = Some(1);
                                }
                                if ui.button("Annuler").clicked() {
                                    action = Some(2);
                                }
                            });
                        }
                        Status::Inconnue => {
                            if ui.button("Annuler").clicked() {
                                action = Some(2);
                            }
                        }
                    }
                }
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    for label in [
                        "Créer une macro",
                        "Expliquer ce bloc",
                        "Corriger cette macro",
                        "Optimiser",
                        "Ajouter une temporisation",
                    ] {
                        if ui.small_button(label).clicked() {
                            send = Some(label.to_owned());
                        }
                    }
                });
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.workshop_chat.input)
                            .hint_text("Décris une action..."),
                    );
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        || ui.button("Envoyer").clicked()
                    {
                        send = Some(std::mem::take(&mut self.workshop_chat.input));
                    }
                });
            });
        self.workshop_chat.open = open;
        if save_mode {
            self.save_preferences();
        }
        if let Some(option) = choice {
            self.workshop_chat.input = format!(
                "{} : {}",
                self.workshop_chat
                    .pending
                    .as_ref()
                    .map_or("", |p| p.request.as_str()),
                option
            );
            self.workshop_chat.pending = None;
        }
        if let Some(action) = action {
            if action == 1 {
                if let Some(p) = self.workshop_chat.pending.take() {
                    self.workshop_chat.input = p.request;
                }
            } else if action == 2 {
                self.workshop_chat.pending = None;
            } else if !stale {
                if let Some(p) = self.workshop_chat.pending.take() {
                    if p.proposal.insertable() {
                        if p.original.is_some() {
                            self.config.keys_mut()[self.selected].steps = p.proposal.steps;
                            self.edited();
                        } else {
                            let steps = &mut self.config.keys_mut()[self.selected].steps;
                            let at = selection
                                .and_then(|s| s.first())
                                .map_or(steps.len(), |i| (i + 1).min(steps.len()));
                            let mut count = 0;
                            for step in p.proposal.steps {
                                if crate::workshop_ops::apply(
                                    steps,
                                    crate::workshop_ops::Edit::Insert(vec![at + count], step),
                                )
                                .is_some()
                                {
                                    count += 1;
                                }
                            }
                            if count > 0 {
                                self.edited();
                            }
                        }
                        self.workshop_chat.say("Modification ajoutée au brouillon ; Ctrl+Z pour annuler. Aucun lancement.");
                    }
                }
            }
        }
        if let Some(text) = send {
            self.workshop_chat_send(text, selection);
        }
    }
}
