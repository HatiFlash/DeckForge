use super::*;
use crate::local_assistant::{self, Proposal, Status};

#[derive(Default)]
pub(super) struct WorkshopChat {
    pub open: bool,
    /// Traduction en cours par l'IA locale : (travail, demande, destination, proposer l'IA connectée en cas d'échec).
    pub thinking: Option<(crate::local_llm::Job, String, (usize, String, usize), bool)>,
    pub input: String,
    pub messages: VecDeque<String>,
    pub pending: Option<ChatPreview>,
    pub consent: Option<ConsentRequest>,
    pub awaiting: Option<ConsentRequest>,
}

pub(super) struct ConsentRequest {
    pub text: String,
    pub destination: serde_json::Value,
    pub blocks: Vec<String>,
    pub scope: (usize, String, usize),
    pub created: std::time::Instant,
}

impl ConsentRequest {
    fn new(text: String, config: &Config, key: usize) -> Self {
        let scope = (config.active, config.page.clone(), key);
        Self {
            text,
            destination: serde_json::json!({"profile":config.profiles[scope.0].name,"page":scope.1,"key":key+1}),
            // Names only: summaries can contain local paths, scripts and secrets.
            blocks: config.keys()[key]
                .steps
                .iter()
                .map(|s| s.name().to_owned())
                .collect(),
            scope,
            created: std::time::Instant::now(),
        }
    }
    /// Destination lisible (« Touche 3 · page Accueil · profil Jeu ») au lieu du JSON transmis.
    pub fn destination_label(&self) -> String {
        let d = &self.destination;
        let page = d["page"].as_str().filter(|p| !p.is_empty()).unwrap_or("principale");
        format!(
            "Touche {} · page {} · profil {}",
            d["key"].as_u64().unwrap_or(0),
            page,
            d["profile"].as_str().unwrap_or("?")
        )
    }
    pub fn public(&self) -> serde_json::Value {
        serde_json::json!({"text":self.text,"destination":self.destination,"blocks":self.blocks,"created":self.created.elapsed().as_secs()})
    }
    fn valid(&self, scope: &(usize, String, usize)) -> bool {
        self.scope == *scope && self.created.elapsed() < Duration::from_secs(600)
    }
}

pub(super) fn mcp_request(
    chat: &mut WorkshopChat,
    scope: &(usize, String, usize),
) -> Option<serde_json::Value> {
    if chat.awaiting.as_ref().is_some_and(|r| !r.valid(scope)) {
        chat.awaiting = None;
    }
    chat.awaiting.as_ref().map(ConsentRequest::public)
}

pub(super) struct ChatPreview {
    pub proposal: Proposal,
    pub scope: (usize, String, usize),
    pub original: Option<Vec<Step>>,
    pub request: String,
    /// Proposition venue de l'IA locale (sinon : règles intégrées).
    pub ai: bool,
}

impl WorkshopChat {
    pub(super) fn say(&mut self, text: impl Into<String>) {
        self.messages
            .push_back(text.into().chars().take(4000).collect());
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
        self.workshop_chat.consent = None;
        self.workshop_chat.awaiting = None;
        self.workshop_chat.thinking = None;
        let scope = (self.config.active, self.config.page.clone(), self.selected);
        let mode = self.preferences.workshop_chat_mode.as_str();
        if mode == "ia" {
            self.workshop_chat.consent =
                Some(ConsentRequest::new(request, &self.config, self.selected));
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
                    ai: false,
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
        if matches!(proposal.status, Status::Inconnue) && self.preferences.local_ai.enabled {
            // Règles muettes : l'IA locale traduit la demande (rien ne quitte le PC).
            self.ask_local_ai(request, scope, mode == "auto");
            return;
        }
        match &proposal.status {
            Status::Inconnue => self.workshop_chat.say("Demande inconnue en local. Exemples : attends 2 s ; ouvre https://exemple.fr ; tape bonjour."),
            Status::Ambigue { question, .. } => self.workshop_chat.say(question.clone()),
            Status::Reconnue => self.workshop_chat.say(format!("Aperçu de {} bloc(s), sans exécution.", proposal.steps.len())),
        }
        if matches!(proposal.status, Status::Inconnue) && mode == "auto" {
            self.workshop_chat.consent = Some(ConsentRequest::new(
                request.clone(),
                &self.config,
                self.selected,
            ));
        }
        self.workshop_chat.pending = Some(ChatPreview {
            proposal,
            scope,
            original: None,
            request,
            ai: false,
        });
    }

    /// Envoie la demande à l'IA locale en arrière-plan (jamais d'insertion sans validation).
    fn ask_local_ai(&mut self, request: String, scope: (usize, String, usize), offer_connected: bool) {
        self.workshop_chat.pending = None;
        self.workshop_chat.say("Je demande à l'IA locale…");
        let job = self.local_ai.spawn(&self.preferences.local_ai, request.clone());
        self.workshop_chat.thinking = Some((job, request, scope, offer_connected));
    }

    pub(super) fn workshop_chat(&mut self, ctx: &egui::Context, selection: Option<&[usize]>) {
        let scope = (self.config.active, self.config.page.clone(), self.selected);
        let stale = self
            .workshop_chat
            .pending
            .as_ref()
            .is_some_and(|p| p.scope != scope);
        mcp_request(&mut self.workshop_chat, &scope);
        self.poll_local_ai(ctx);
        if self
            .workshop_chat
            .consent
            .as_ref()
            .is_some_and(|r| !r.valid(&scope))
        {
            self.workshop_chat.consent = None;
        }
        if self.proposals.pending.front().is_some() && self.workshop_chat.awaiting.is_some() {
            self.workshop_chat.awaiting = None;
            self.workshop_chat
                .say("Proposition de ton IA reçue. Vérifie-la dans la fenêtre de validation.");
        }
        if !self.workshop_chat.open {
            return;
        }
        let mut open = self.workshop_chat.open;
        let mut send = None;
        let mut choice = None;
        let mut action = None;
        let mut save_mode = false;
        let mut consent_action = None;
        let screen = ctx.screen_rect();
        let chat_content = |ui: &mut egui::Ui| {
            // egui n'enroule pas les textes dans les rangées horizontales par défaut : ils
            // débordaient du panneau (messages, destination, boutons coupés).
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.horizontal_wrapped(|ui| {
                iconset::heading(ui, Ico::Robot, RichText::new("Assistant · Atelier").strong());
                if ui.button("Retour à la séquence").clicked() {
                    open = false;
                }
            });
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.workshop_chat.input)
                        .hint_text("Décris une action...")
                        .desired_width((ui.available_width() - 84.0).max(60.0)),
                );
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    || ui.button("Envoyer").clicked()
                {
                    send = Some(std::mem::take(&mut self.workshop_chat.input));
                }
            });
            // Raccourcis toujours visibles sous la saisie (avant : en bas du défilement, hors écran).
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
            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
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
                    if let Some(request) = &self.workshop_chat.consent {
                        ui.separator();
                        ui.label(RichText::new("Demande inconnue en local : la transmettre à ton IA ?").strong());
                        // Choix en premier : visible sans défiler, même sur petit écran.
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Transmettre à mon IA").clicked() {
                                consent_action = Some(true);
                            }
                            if ui.button("Non merci").clicked() {
                                consent_action = Some(false);
                            }
                        });
                        let muted = theme::palette(ctx).muted;
                        ui.label(RichText::new("Contexte transmis uniquement après ton accord :").small().color(muted));
                        ui.label(RichText::new(format!("Demande : {}", request.text)).small());
                        ui.label(RichText::new(format!("Destination : {}", request.destination_label())).small());
                        ui.label(RichText::new(format!(
                            "Blocs actuels (types uniquement) : {}",
                            request.blocks.join(", ")
                        )).small());
                        ui.label(RichText::new("Aucun chemin personnel ni secret des blocs n'est transmis.").small().color(muted));
                    }
                    if self.workshop_chat.consent.is_some() || self.workshop_chat.awaiting.is_some()
                    {
                        if !self.assistant.as_ref().is_some_and(|s| s.enabled()) {
                            ui.label("Indisponible");
                            if ui.button("Connecter une IA").clicked() {
                                self.connect_window = true;
                            }
                        }
                    }
                    if let Some((job, _, _, _)) = &self.workshop_chat.thinking {
                        ui.horizontal_wrapped(|ui| {
                            crate::motion::loader(ui, 16.0);
                            ui.label(format!("L'IA locale réfléchit… {} s", job.started.elapsed().as_secs()));
                            if ui.button("Annuler").clicked() {
                                consent_action = Some(false);
                            }
                        });
                    }
                    if self.workshop_chat.awaiting.is_some() {
                        ui.label("En attente de ton IA…");
                        if ui.button("Annuler la demande IA").clicked() {
                            consent_action = Some(false);
                        }
                    }
                    if let Some(p) = self.proposals.pending.front() {
                        egui::Frame::new()
                            .fill(theme::palette(ctx).surface)
                            .corner_radius(8)
                            .inner_margin(8)
                            .show(ui, |ui| {
                                ui.label(RichText::new("Proposition de ton IA reçue").strong());
                                ui.label(&p.title);
                                ui.label(p.details.chars().take(180).collect::<String>());
                            });
                        if ui.button("Voir et valider la proposition").clicked() {
                            self.workshop_chat.open = false;
                        }
                    }
                    // Aperçu et boutons Ajouter/Annuler avant l'historique : visibles sans défiler.
                    if let Some(pending) = &self.workshop_chat.pending {
                        ui.separator();
                        if stale {
                            ui.colored_label(
                                theme::palette(ctx).muted,
                                "Proposition périmée : touche, page ou profil changé.",
                            );
                        }
                        if self.preferences.local_ai.enabled
                            && !pending.ai
                            && pending.original.is_none()
                            && !stale
                            && self.workshop_chat.thinking.is_none()
                            && ui
                                .small_button("Pas ça ? Demander plutôt à l'IA locale")
                                .on_hover_text("Les règles intégrées ont répondu ; l'IA locale peut mieux comprendre une demande libre. Rien ne quitte le PC.")
                                .clicked()
                        {
                            action = Some(3);
                        }
                        match &pending.proposal.status {
                            Status::Ambigue { question, choix } => {
                                ui.label(question);
                                if self.preferences.workshop_chat_mode == "auto"
                                    && !stale
                                    && ui.button("Demander à mon IA").clicked()
                                {
                                    consent_action = Some(true);
                                    self.workshop_chat.consent = Some(ConsentRequest::new(
                                        pending.request.clone(),
                                        &self.config,
                                        self.selected,
                                    ));
                                }
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
                                            egui::Frame::new()
                                                .fill(theme::palette(ctx).surface)
                                                .corner_radius(8)
                                                .inner_margin(7)
                                                .show(ui, |ui| {
                                                    iconset::heading(ui, iconset::for_step(step), RichText::new(step.name()).strong());
                                                    ui.label(super::extras::block_summary(step));
                                                });
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
                                ui.horizontal_wrapped(|ui| {
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
                    egui::ScrollArea::vertical()
                        .id_salt("chat_history")
                        .max_height((screen.height() * 0.32).clamp(120.0, 320.0))
                        .show(ui, |ui| {
                            for message in &self.workshop_chat.messages {
                                let user = message.starts_with("Vous : ");
                                ui.with_layout(
                                    egui::Layout::top_down(if user {
                                        egui::Align::Max
                                    } else {
                                        egui::Align::Min
                                    }),
                                    |ui| {
                                        egui::Frame::new()
                                            .fill(if user {
                                                theme::accent().gamma_multiply(0.35)
                                            } else {
                                                theme::palette(ctx).surface
                                            })
                                            .corner_radius(8)
                                            .inner_margin(8)
                                            .show(ui, |ui| {
                                                ui.set_max_width((ui.available_width() - 16.0).max(80.0));
                                                ui.add(egui::Label::new(message).wrap());
                                            });
                                    },
                                );
                            }
                        });
                });
        };
        // Reserve room for the assistant before laying out the canvas at every size.
        // Panneau redimensionnable (bord gauche) : largeur par défaut adaptée à l'écran.
        let max_width = (screen.width() * 0.5).max(300.0);
        egui::SidePanel::right("workshop_chat_panel")
            .resizable(true)
            .default_width((screen.width() * 0.34).clamp(300.0, 420.0))
            .width_range(280.0..=max_width)
            .show(ctx, chat_content);
        self.workshop_chat.open = open;
        if let Some(agree) = consent_action {
            if agree {
                if let Some(request) = self
                    .workshop_chat
                    .consent
                    .take()
                    .filter(|r| r.valid(&scope))
                {
                    if self.assistant.as_ref().is_some_and(|s| s.enabled()) {
                        self.assistant_request = request.text.clone();
                        ctx.copy_text(self.automation_prompt());
                        self.workshop_chat.pending = None;
                        self.workshop_chat.awaiting = Some(request);
                        self.workshop_chat
                            .say("Demande copiée. Colle-la dans ton client IA MCP.");
                    } else {
                        self.workshop_chat
                            .say("Indisponible : active et connecte ton IA MCP.");
                        self.connect_window = true;
                    }
                }
            } else {
                self.workshop_chat.consent = None;
                self.workshop_chat.awaiting = None;
                if self.workshop_chat.thinking.take().is_some() {
                    self.workshop_chat.say("Demande à l'IA locale annulée.");
                }
            }
        }
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
            } else if action == 3 {
                if let Some(p) = self.workshop_chat.pending.take() {
                    let offer = self.preferences.workshop_chat_mode == "auto";
                    self.ask_local_ai(p.request, p.scope, offer);
                }
            } else if !stale {
                if let Some(p) = self.workshop_chat.pending.take() {
                    let p_missing = p.proposal.missing.clone();
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
                        let todo: Vec<String> =
                            p_missing.iter().map(|m| m.reason.clone()).collect();
                        if todo.is_empty() {
                            self.workshop_chat.say("Modification ajoutée au brouillon ; Ctrl+Z pour annuler. Aucun lancement.");
                        } else {
                            self.workshop_chat.say(format!("Ajouté en brouillon. À compléter dans les propriétés : {}. Ctrl+Z pour annuler.", todo.join(", ")));
                        }
                    }
                }
            }
        }
        if let Some(text) = send {
            self.workshop_chat_send(text, selection);
        }
    }

    /// Récupère la traduction de l'IA locale et la montre en aperçu (jamais insérée d'office).
    fn poll_local_ai(&mut self, ctx: &egui::Context) {
        let Some((job, _, _, _)) = &self.workshop_chat.thinking else { return };
        let Some(result) = job.poll() else {
            ctx.request_repaint_after(Duration::from_millis(250));
            return;
        };
        let (_, request, scope, offer_ai) = self.workshop_chat.thinking.take().unwrap();
        let current = (self.config.active, self.config.page.clone(), self.selected);
        if scope != current {
            self.workshop_chat.say("Réponse de l'IA locale ignorée : la touche sélectionnée a changé.");
            return;
        }
        let proposal = match result {
            Ok(steps) => local_assistant::from_model(steps, &self.config),
            Err(error) => {
                self.workshop_chat.say(format!("IA locale indisponible : {error}"));
                local_assistant::from_model(vec![], &self.config)
            }
        };
        match &proposal.status {
            Status::Reconnue => self.workshop_chat.say(format!(
                "Aperçu de {} bloc(s) proposé par l'IA locale, sans exécution.",
                proposal.steps.len()
            )),
            _ => {
                self.workshop_chat.say("L'IA locale n'a pas trouvé de blocs pour cette demande. Reformule ou ajoute les blocs à la main.");
                if offer_ai {
                    self.workshop_chat.consent = Some(ConsentRequest::new(request.clone(), &self.config, self.selected));
                }
            }
        }
        self.workshop_chat.pending = Some(ChatPreview { proposal, scope, original: None, request, ai: true });
    }

    /// Réglages → IA locale : moteur, modèle et test de connexion. Aucun téléchargement implicite.
    pub(super) fn local_ai_settings(&mut self, ui: &mut egui::Ui, p: &theme::Palette) {
        ui.label(
            RichText::new("Comprend les demandes libres en français et en anglais quand les règles intégrées ne suffisent pas. Tout reste sur ce PC ; chaque proposition est montrée avant ajout. Environ 1,2 Go de mémoire vidéo (ou de RAM).")
                .small()
                .color(p.muted),
        );
        let mut changed = ui.checkbox(&mut self.preferences.local_ai.enabled, "Utiliser une IA locale").changed();
        ui.horizontal_wrapped(|ui| {
            ui.label("Moteur");
            for (id, label) in [("integre", "Intégré à DeckForge"), ("ollama", "Ollama")] {
                if ui.selectable_label(self.preferences.local_ai.backend == id, label).clicked() {
                    self.preferences.local_ai.backend = id.into();
                    self.local_ai.stop();
                    changed = true;
                }
            }
        });
        if self.preferences.local_ai.backend == "ollama" {
            ui.horizontal_wrapped(|ui| {
                ui.label("Adresse");
                changed |= ui.add(egui::TextEdit::singleline(&mut self.preferences.local_ai.endpoint).desired_width(190.0)).lost_focus();
                ui.label("Modèle");
                changed |= ui.add(egui::TextEdit::singleline(&mut self.preferences.local_ai.model).desired_width(120.0)).lost_focus();
            });
            ui.label(RichText::new(format!("Installer Ollama (ollama.com), puis dans un terminal : ollama pull {}", crate::local_llm::OLLAMA_MODEL)).small().color(p.muted));
        } else {
            let server = crate::local_llm::builtin_server().exists();
            let model = crate::local_llm::builtin_model().exists();
            ui.label(RichText::new(match (server, model) {
                (false, _) => "Moteur intégré pas encore installé.",
                (true, false) => "Moteur présent ; modèle Qwen2.5 1,5B à télécharger.",
                (true, true) => "Moteur et modèle prêts.",
            }).size(12.0).color(p.muted));
            if server && model {
                let state = if self.local_ai.loaded() {
                    format!("En mémoire ({}) ; libéré automatiquement après 10 min sans demande.", self.local_ai.mode.lock().map(|m| m.clone()).unwrap_or_default())
                } else {
                    "Pas en mémoire : démarre à la première demande (quelques secondes).".into()
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(state).size(12.0).color(p.muted));
                    if self.local_ai.loaded() && ui.small_button("Libérer maintenant").clicked() {
                        self.local_ai.stop();
                    }
                });
            }
            self.local_ai_download_ui(ui, p, !(server && model));
        }
        ui.horizontal_wrapped(|ui| {
            let busy = self.local_ai_check.is_some();
            if ui.add_enabled(!busy && self.local_ai_install.is_none(), egui::Button::new("Tester")).clicked() {
                self.local_ai_status = "Test en cours…".into();
                self.local_ai_check = Some(self.local_ai.check(&self.preferences.local_ai));
            }
            if busy {
                crate::motion::loader(ui, 16.0);
            }
            if let Some(result) = self.local_ai_check.as_ref().and_then(|j| j.poll()) {
                self.local_ai_check = None;
                self.local_ai_status = match result {
                    Ok(steps) if !steps.is_empty() => "IA locale opérationnelle.".into(),
                    Ok(_) => "Connexion établie, mais réponse vide du modèle.".into(),
                    Err(e) => format!("Échec : {e}"),
                };
            } else if busy {
                ui.ctx().request_repaint_after(Duration::from_millis(250));
            }
            if !self.local_ai_status.is_empty() {
                ui.label(RichText::new(&self.local_ai_status).small());
            }
        });
        if changed {
            self.save_preferences();
        }
    }

    /// Installation du moteur intégré : uniquement sur clic, fichiers épinglés et vérifiés (SHA-256).
    fn local_ai_download_ui(&mut self, ui: &mut egui::Ui, p: &theme::Palette, needed: bool) {
        if let Some(install) = &self.local_ai_install {
            match install.poll() {
                Some(result) => {
                    self.local_ai_install = None;
                    self.local_ai_status = match result {
                        Ok(()) => "Moteur et modèle installés. Clique sur Tester.".into(),
                        Err(e) if e == crate::local_llm::CANCELLED => "Installation annulée ; elle reprendra là où elle s'est arrêtée.".into(),
                        Err(e) => format!("Installation impossible : {e}"),
                    };
                }
                None => {
                    let (stage, received, total) = install.progress();
                    let mut cancel = false;
                    ui.horizontal_wrapped(|ui| {
                        crate::motion::loader(ui, 16.0);
                        let fraction = if total > 0 { (received as f32 / total as f32).min(1.0) } else { 0.0 };
                        ui.add(egui::ProgressBar::new(fraction).desired_width(180.0).text(format!("{stage} : {} / {} Mo", received / 1_000_000, total / 1_000_000)));
                        cancel = ui.button("Annuler").on_hover_text("Le téléchargement reprendra là où il s'est arrêté.").clicked();
                    });
                    if cancel {
                        install.cancel();
                    }
                    ui.ctx().request_repaint_after(Duration::from_millis(500));
                    return;
                }
            }
        }
        if needed {
            ui.label(RichText::new("Téléchargement unique depuis GitHub (llama.cpp, 37 Mo) et Hugging Face (Qwen2.5 1,5B, 1,1 Go), vérifié par empreinte SHA-256, installé dans %LOCALAPPDATA%\\DeckForge\\ai. Annulable ; reprend là où il s'est arrêté.").size(12.0).color(p.muted));
            let dir = crate::local_llm::builtin_dir();
            let (need, free) = (crate::local_llm::space_needed(&dir), crate::local_llm::free_space(dir.ancestors().find(|d| d.exists()).unwrap_or(&dir)));
            let enough = free.is_none_or(|f| f >= need);
            if !enough {
                ui.colored_label(theme::WARNING, format!("Espace disque insuffisant : {} Mo nécessaires, {} Mo libres.", need / 1_000_000, free.unwrap_or(0) / 1_000_000));
            }
            if ui.add_enabled(enough, egui::Button::new("Installer le moteur et le modèle (≈1,2 Go)")).clicked() {
                self.local_ai.stop();
                self.local_ai_status.clear();
                self.local_ai_install = Some(crate::local_llm::install_builtin());
            }
        }
    }
}

#[cfg(test)]
mod consent_tests {
    use super::*;

    #[test]
    fn pending_request_never_appears_before_consent_and_clears_on_cancel_and_expiry() {
        let config = Config::default();
        let scope = (config.active, config.page.clone(), 0);
        let mut chat = WorkshopChat::default();
        chat.consent = Some(ConsentRequest::new("aide".into(), &config, 0));
        assert!(mcp_request(&mut chat, &scope).is_none());
        chat.awaiting = chat.consent.take();
        assert_eq!(mcp_request(&mut chat, &scope).unwrap()["text"], "aide");
        chat.awaiting = None;
        assert!(mcp_request(&mut chat, &scope).is_none());
        let mut request = ConsentRequest::new("aide".into(), &config, 0);
        request.created -= Duration::from_secs(601);
        chat.awaiting = Some(request);
        assert!(mcp_request(&mut chat, &scope).is_none());
        assert!(chat.awaiting.is_none());
    }

    #[test]
    fn request_exports_only_block_names_not_parameters_or_paths() {
        let mut config = Config::default();
        config.keys_mut()[0].steps.push(Step::Launch {
            program: "C:\\Private\\secret.exe".into(),
            args: vec!["password".into()],
            focus_if_running: false,
        });
        let request = ConsentRequest::new("ouvre".into(), &config, 0)
            .public()
            .to_string();
        assert!(!request.contains("Private"));
        assert!(!request.contains("password"));
        assert!(request.contains("destination"));
    }
}
