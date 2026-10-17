use super::*;

pub(super) fn step_mut<'a>(steps: &'a mut [Step], path: &[usize]) -> Option<&'a mut Step> {
    let step = steps.get_mut(*path.first()?)?;
    if path.len() == 1 {
        return Some(step);
    }
    let branch = *path.get(1)?;
    let children = match step {
        Step::Repeat { steps, .. } if branch == 0 => steps,
        Step::IfWindow {
            then_steps,
            else_steps,
            ..
        }
        | Step::IfVariable {
            then_steps,
            else_steps,
            ..
        } => {
            if branch == 0 {
                then_steps
            } else {
                else_steps
            }
        }
        _ => return None,
    };
    step_mut(children, &path[2..])
}
pub(super) fn block_editor(
    ui: &mut egui::Ui,
    steps: &mut Vec<Step>,
    profiles: &[String],
    pages: &[crate::model::Page],
    audio: &crate::audio::Snapshot,
    parent: &[usize],
    record: &mut Option<Vec<usize>>,
    depth: usize,
    running: &[usize],
    pick_audio: &mut Option<(Vec<usize>, usize)>,
) -> bool {
    let p = theme::palette(ui.ctx());
    let mut changed = false;
    let mut remove = None;
    let mut movement = None;
    for (i, step) in steps.iter_mut().enumerate() {
        let mut path = parent.to_vec();
        path.push(i);
        ui.push_id(i, |ui| {
            if i > 0 {
                ui.label(RichText::new("↓").color(p.muted));
            }
            let color = if matches!(step, Step::IfWindow { .. } | Step::IfVariable { .. }) {
                Color32::from_rgb(164, 123, 239)
            } else if matches!(step, Step::Repeat { .. }) {
                Color32::from_rgb(239, 169, 83)
            } else {
                BLUE
            };
            egui::Frame::new()
                .fill(p.bg)
                .stroke(egui::Stroke::new(
                    if running == path { 2.0_f32 } else { 1.0_f32 },
                    if running == path { GREEN } else { p.line },
                ))
                .corner_radius(12)
                .inner_margin(14)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        theme::pill(
                            ui,
                            &path
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| i % 2 == 0)
                                .map(|(_, n)| (n + 1).to_string())
                                .collect::<Vec<_>>()
                                .join("."),
                            color,
                        );
                        ui.label(RichText::new(step.name()).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Retirer").clicked() {
                                remove = Some(i);
                            }
                            if ui
                                .add_enabled(i > 0, egui::Button::new("↑").small())
                                .clicked()
                            {
                                movement = Some((i, i - 1));
                            }
                            if ui.small_button("↓").clicked() {
                                movement = Some((i, i + 1));
                            }
                        });
                    });
                    ui.add_space(6.0);
                    let mut capture = false;
                    let mut icon = None;
                    changed |= edit_step(ui, step, profiles, pages, &mut capture, audio, &mut icon);
                    if let Some(state) = icon {
                        *pick_audio = Some((path.clone(), state));
                    }
                    if capture {
                        *record = Some(path.clone());
                    }
                    match step {
                        Step::IfWindow {
                            then_steps,
                            else_steps,
                            ..
                        }
                        | Step::IfVariable {
                            then_steps,
                            else_steps,
                            ..
                        } => {
                            for (branch, label, list) in
                                [(0, "ALORS", then_steps), (1, "SINON", else_steps)]
                            {
                                ui.add_space(8.0);
                                ui.label(RichText::new(label).small().strong().color(color));
                                let mut child = path.clone();
                                child.push(branch);
                                ui.indent(branch, |ui| {
                                    changed |= block_editor(
                                        ui,
                                        list,
                                        profiles,
                                        pages,
                                        audio,
                                        &child,
                                        record,
                                        depth + 1,
                                        running,
                                        pick_audio,
                                    );
                                });
                            }
                        }
                        Step::Repeat { steps, .. } => {
                            let mut child = path.clone();
                            child.push(0);
                            ui.indent("loop", |ui| {
                                changed |= block_editor(
                                    ui,
                                    steps,
                                    profiles,
                                    pages,
                                    audio,
                                    &child,
                                    record,
                                    depth + 1,
                                    running,
                                    pick_audio,
                                );
                            });
                        }
                        _ => {}
                    }
                });
        });
    }
    if let Some(i) = remove {
        steps.remove(i);
        changed = true;
    }
    if let Some((a, b)) = movement {
        if b < steps.len() {
            steps.swap(a, b);
            changed = true;
        }
    }
    if depth <= 8 && steps.len() < 256 {
        ui.menu_button("+ Ajouter un bloc", |ui| {
            for (i, label) in crate::automation::NAMES.iter().enumerate() {
                if i == 12 || depth > 0 && (7..=11).contains(&i) {
                    continue;
                }
                if ui.button(*label).clicked() {
                    steps.push(
                        catalog::action(
                            i,
                            profiles.first().map(String::as_str).unwrap_or("Bureau"),
                        )
                        .steps
                        .remove(0),
                    );
                    changed = true;
                    ui.close_menu();
                }
            }
        });
    }
    changed
}
impl App {
    pub(super) fn automation_prompt(&self) -> String {
        let profile = &self.config.profiles[self.config.active].name;
        let destination =
            serde_json::json!({"profile":profile,"page":self.config.page,"key":self.selected+1});
        format!("Utilise le connecteur MCP DeckForge pour proposer cette automatisation :\n{}\n\nDestination exacte : {destination}\nLis d’abord deckforge_get_state et deckforge_get_profile. Si la demande concerne l’audio, utilise deckforge_get_audio_devices pour récupérer les identifiants réels. Tu peux utiliser deckforge_get_icons ; pour audio_switch, first_icon/second_icon acceptent builtin:headphones, builtin:speakers et builtin:microphone. Crée les étapes avec deckforge_configure_key (conditions, répétitions et variables possibles). Si une information manque, demande-la-moi au lieu d’inventer un périphérique, un programme ou un script. N’exécute aucune macro. Attends la validation de la proposition dans DeckForge ; les blocs resteront ensuite modifiables dans l’Atelier.",self.assistant_request.trim())
    }
    pub(super) fn presented_key(&self, index: usize) -> Key {
        crate::audio::present_key(&self.config.keys()[index], &self.audio)
    }
    pub(super) fn refresh_hotkeys(&self) {
        if let Some(desktop) = &self.desktop {
            if self.recording.is_some() || self.trigger_recording {
                let _ = desktop.tx.send(crate::desktop::Command::Hotkeys(vec![]));
                return;
            }
            let name = &self.config.profiles[self.config.active].name;
            let bindings = self
                .preferences
                .triggers
                .iter()
                .filter(|t| &t.profile == name && t.page == self.config.page && t.key < 15)
                .map(|t| (t.key, t.keys.clone()))
                .collect();
            let _ = desktop.tx.send(crate::desktop::Command::Hotkeys(bindings));
        }
    }
    pub(super) fn extras(&mut self, ctx: &egui::Context) {
        if self.open_requested {
            self.open_requested = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.workflow(ctx);
        self.asset_browser(ctx);
        self.desktop_settings(ctx);
    }
    fn workflow(&mut self, ctx: &egui::Context) {
        if !self.show_workflow {
            return;
        }
        let mut open = true;
        let mut record = None;
        let mut pick_audio = None;
        let mut changed = false;
        let mut export = false;
        let title = format!(
            "Atelier · touche {:02} · {}",
            self.selected + 1,
            self.config.keys()[self.selected].label
        );
        egui::Window::new(title).id(egui::Id::new("workflow04")).open(&mut open).default_width(800.0).default_height(610.0).min_width(520.0).show(ctx,|ui|{
            ui.label(RichText::new("Construis ton automatisation").size(23.0).strong());
            ui.label("Assemble les blocs de haut en bas. Les variables sont propres à chaque exécution.");
            ui.horizontal(|ui|{if ui.button("Créer avec mon IA").clicked(){self.assistant_settings=true;}if ui.button("Simuler les blocs").clicked(){self.preview(true);}if ui.button("Tester dans 3 s").clicked(){self.preview(false);}if ui.button("Exporter en AutoHotkey v2…").clicked(){export=true;}});
            if !self.engine.running.load(Ordering::SeqCst){self.running_block.clear();}
            let profiles=self.config.profiles.iter().map(|p|p.name.clone()).collect::<Vec<_>>();let pages=self.config.profiles[self.config.active].pages.clone();let selected=self.selected;
            egui::ScrollArea::vertical().id_salt("workflow_scroll").max_height(470.0).show(ui,|ui|{
                changed=block_editor(ui,&mut self.config.keys_mut()[selected].steps,&profiles,&pages,&self.audio,&[],&mut record,0,&self.running_block,&mut pick_audio);
            });
            if let Err(e)=validate_key(&self.config.keys()[selected],&self.config){ui.colored_label(Color32::LIGHT_RED,format!("Brouillon : {e}"));}
        });
        if let Some(target) = pick_audio {
            self.open_icon_gallery(Some(target));
        }
        self.show_workflow = open;
        if changed {
            self.edited();
        }
        if let Some(path) = record {
            self.recording = Some((
                self.config.active,
                self.config.page.clone(),
                self.selected,
                path,
                self.gate.enabled,
            ));
            self.gate.enabled = false;
            self.recording_error.clear();
            self.engine.stop();
            self.refresh_hotkeys();
        }
        if export {
            match crate::automation::export(&self.config.keys()[self.selected]) {
                Ok(text) => {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("automatisation.ahk")
                        .add_filter("Script AutoHotkey v2", &["ahk"])
                        .save_file()
                    {
                        match std::fs::write(path, text) {
                            Ok(()) => self.log("Script AutoHotkey exporté.".into()),
                            Err(e) => self.fail(e.to_string()),
                        }
                    }
                }
                Err(e) => self.fail(e.to_string()),
            }
        }
    }
    pub(super) fn reload_icons(&mut self) {
        self.available_icons = crate::assets::scan(&[self.path.parent().unwrap().join("assets")]);
        self.available_plugins = crate::assets::plugins();
    }
    fn desktop_settings(&mut self, ctx: &egui::Context) {
        if !self.show_desktop_settings {
            return;
        }
        let mut open = true;
        let mut changed = false;
        let mut startup = None;
        let mut hotkeys = false;
        let mut capture = false;
        egui::Window::new("Windows et raccourcis").open(&mut open).default_width(510.0).show(ctx,|ui|{
            let mut enabled=self.preferences.start_with_windows;
            if ui.checkbox(&mut enabled,"Démarrer avec Windows en arrière-plan").changed(){startup=Some(enabled);}
            changed|=ui.checkbox(&mut self.preferences.close_to_tray,"Fermer la fenêtre garde DeckForge près de l’horloge").changed();
            if ui.add_enabled(self.tray_ready,egui::Button::new("Masquer maintenant")).clicked(){ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));}
            ui.separator();ui.label(RichText::new(format!("Raccourci global · touche {:02}",self.selected+1)).strong());
            ui.label("Actif sur la page actuellement ouverte. Les raccourcis restent disponibles sans D6 branché.");
            let name=self.config.profiles[self.config.active].name.clone();let page=self.config.page.clone();let key=self.selected;
            let index=self.preferences.triggers.iter().position(|t|t.profile==name&&t.page==page&&t.key==key);
            let mut text=index.map(|i|self.preferences.triggers[i].keys.clone()).unwrap_or_default();
            ui.horizontal(|ui|{if ui.text_edit_singleline(&mut text).changed(){hotkeys=true;}if ui.button("Enregistrer…").clicked(){capture=true;}});
            if !text.is_empty(){if let Err(e)=crate::desktop::chord(&text){ui.colored_label(Color32::LIGHT_RED,e.to_string());}}
            ui.label(RichText::new("Exemple : CTRL+ALT+F9. Vide le champ pour désactiver. Windows signale les combinaisons déjà utilisées dans le journal.").small());
            if hotkeys {if let Some(i)=index {if text.is_empty(){self.preferences.triggers.remove(i);}else{self.preferences.triggers[i].keys=text;}}else if !text.is_empty()&&self.preferences.triggers.len()<256{self.preferences.triggers.push(crate::desktop::Trigger{profile:name,page,key,keys:text});}changed=true;}
            if self.trigger_recording{ui.colored_label(BLUE,"Appuie sur la combinaison souhaitée…");if ui.button("Annuler").clicked(){self.trigger_recording=false;self.refresh_hotkeys();}}
        });
        self.show_desktop_settings = open;
        if !open && self.trigger_recording {
            self.trigger_recording = false;
            self.refresh_hotkeys();
        }
        if capture {
            self.trigger_recording = true;
            self.engine.stop();
            self.refresh_hotkeys();
        }
        if let Some(enable) = startup {
            match crate::desktop::startup(enable) {
                Ok(()) => {
                    self.preferences.start_with_windows = enable;
                    changed = true;
                    self.log("Démarrage Windows mis à jour.".into());
                }
                Err(e) => self.fail(e.to_string()),
            }
        }
        if changed && self.save_preferences() {
            self.refresh_hotkeys();
        }
    }
    pub(super) fn record_trigger(&mut self, ctx: &egui::Context) {
        if !self.trigger_recording || !ctx.input(|i| i.focused) {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        if let Some(result) = crate::recorder::combination(&events) {
            ctx.input_mut(|i| i.events.clear());
            match result {
                Ok(keys) => {
                    if let Err(e) = crate::desktop::chord(&keys) {
                        self.fail(e.to_string());
                        return;
                    }
                    let name = self.config.profiles[self.config.active].name.clone();
                    let page = self.config.page.clone();
                    let key = self.selected;
                    if let Some(t) = self
                        .preferences
                        .triggers
                        .iter_mut()
                        .find(|t| t.profile == name && t.page == page && t.key == key)
                    {
                        t.keys = keys;
                    } else if self.preferences.triggers.len() < 256 {
                        self.preferences.triggers.push(crate::desktop::Trigger {
                            profile: name,
                            page,
                            key,
                            keys,
                        });
                    }
                    self.trigger_recording = false;
                    if self.save_preferences() {
                        self.refresh_hotkeys();
                    }
                }
                Err(e) => self.fail(e),
            }
        }
    }
}
