use super::*;

// These two presets send hotkeys; Discord does not report its real mute state.
pub(super) fn discord_shortcut(key: &Key) -> Option<&str> {
    let expected = match key.label.as_str() {
        "Discord : Micro" => "CTRL+SHIFT+F13",
        "Discord : Casque" => "CTRL+SHIFT+F14",
        _ => return None,
    };
    match key.steps.as_slice() {
        [Step::Hotkey { keys }] if keys == expected => Some(keys.as_str()),
        _ => None,
    }
}

#[cfg(test)]
mod discord_assistant_tests {
    use super::*;
    #[test]
    fn only_original_discord_shortcuts_show_the_assistant() {
        let mut key = Key { label: "Discord : Micro".into(),
            steps: vec![Step::Hotkey { keys: "CTRL+SHIFT+F13".into() }],
            ..Key::default() };
        assert_eq!(discord_shortcut(&key), Some("CTRL+SHIFT+F13"));
        key.steps = vec![Step::Hotkey { keys: "CTRL+SHIFT+F14".into() }];
        assert_eq!(discord_shortcut(&key), None);
        key.label = "Discord : Casque".into();
        assert_eq!(discord_shortcut(&key), Some("CTRL+SHIFT+F14"));
        key.steps.push(Step::Delay { ms: 1 });
        assert_eq!(discord_shortcut(&key), None);
    }
}
pub(super) fn step_mut<'a>(steps: &'a mut [Step], path: &[usize]) -> Option<&'a mut Step> {
    let step = steps.get_mut(*path.first()?)?.active_mut();
    if path.len() == 1 {
        return Some(step);
    }
    let branch = *path.get(1)?;
    let children = match step {
        Step::Repeat { steps, .. } if branch == 0 => steps,
        Step::Toggle { a, b } => if branch == 0 { a } else { b },
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
const BLOCK_CATEGORIES: &[(&str, &[usize])] = &[
    ("Clavier", &[0]),
    ("Streaming", &[19, 20, 21, 22, 23, 24, 25, 30, 31, 0, 4]), ("Système", &[0, 2, 4, 26, 27, 28, 29, 32, 33]),
    ("Texte", &[1]), ("Applications et sites", &[2, 3]),
    ("Médias et audio", &[4, 13]), ("Fenêtres", &[6]), ("Attendre", &[5]),
    ("Logique et répétitions", &[14, 15]), ("Variables", &[16, 17]),
    ("Navigation DeckForge", &[7, 8, 9, 10, 11]), ("Avancé / AutoHotkey", &[18]),
];
fn block_help(kind: usize) -> &'static str {
    match kind {
        0 => "Appuie sur une combinaison de touches.", 1 => "Écrit du texte dans la fenêtre active.",
        2 => "Démarre un programme Windows.", 3 => "Ouvre une adresse web.",
        4 => "Commande la lecture et le volume.", 5 => "Attend avant le bloc suivant.",
        6 => "Attend et peut activer une fenêtre.", 13 => "Bascule entre deux appareils audio.",
        14 => "Exécute des blocs si une fenêtre est ouverte.", 15 => "Rejoue plusieurs fois les blocs imbriqués.",
        16 => "Enregistre une valeur pour cette exécution.", 17 => "Choisit des blocs selon une valeur.",
        19..=25 => "Commande OBS Studio par WebSocket local (OBS 28 ou plus).",
        32 => "Règle ou ajuste la luminosité du D6 (0 à 100 %).",
        33 => "Met le D6 en veille ; le prochain appui le réveille.",
        18 => "Exécute un script AutoHotkey v2.", _ => "Change la page, le dossier ou le profil affiché.",
    }
}
fn matches_palette(kind: usize, category: &str, search: &str) -> bool {
    let aliases = match kind {
        19..=25 => "obs studio record stream scene source visibility virtual camera replay mute",
        0 => "hotkey super hotkeys discord browser back forward refresh quick control close window",
        1 => "text plain text",
        2 => "open openapps quick tool calculator mail music",
        3 => "website home page",
        4 => "multimedia systemvolume mute quick control",
        7..=11 => "pages folders",
        15 => "multi action",
        _ => "",
    };
    search.is_empty() || format!("{} {} {}", category, crate::automation::NAMES[kind], aliases).to_lowercase().contains(&search.to_lowercase())
}
fn palette(ui: &mut egui::Ui, search: &mut String, depth: usize, advanced: bool) -> Option<usize> {
    let mut selected = None;
    ui.add(egui::TextEdit::singleline(search).hint_text("Rechercher un bloc"));
    for &(category, kinds) in BLOCK_CATEGORIES {
        let matches: Vec<_> = kinds.iter().copied().filter(|&kind| {
            (depth == 0 || !(7..=11).contains(&kind)) &&
            (advanced || kind != 18) &&
            matches_palette(kind, category, search)
        }).collect();
        if matches.is_empty() { continue; }
        ui.label(RichText::new(category).strong().color(BLUE));
        for kind in matches {
            if ui.button(crate::automation::NAMES[kind]).on_hover_text(block_help(kind)).clicked() {
                selected = Some(kind);
            }
        }
        ui.add_space(7.0);
    }
    selected
}
fn simple_fields_visible(step: &Step, advanced: bool) -> bool {
    advanced || !matches!(step.active(), Step::AutoHotkey { .. })
}
fn block_summary(step: &Step) -> String {
    match step.active() {
        Step::ObsScene { name } => format!("OBS : scène {name}"),
        Step::ObsRecord { mode } | Step::ObsStream { mode } | Step::ObsVirtualCam { mode } | Step::ObsReplay { mode } => format!("{} : {mode}", step.name()),
        Step::ObsMute { input, mode } => format!("OBS : {input} ({mode})"),
        Step::ObsSourceVisibility { scene, source, mode } => format!("OBS : {scene} / {source} ({mode})"),
        Step::Hotkey { keys } => format!("Appuyer sur {keys}"),
        Step::Delay { ms } => format!("Attendre {ms} ms"),
        Step::Url { url } => format!("Ouvrir {url}"),
        Step::Text { text } => format!("Écrire {}", text.chars().take(50).collect::<String>()),
        Step::Launch { program, .. } => format!("Lancer {program}"),
        Step::Toggle { a, b } => format!("Basculer : {} / {} blocs", a.len(), b.len()),
        Step::Repeat { count, steps } => format!("Répéter {count} fois ({} blocs)", steps.len()),
        Step::UdpSend { host, port, .. } => format!("UDP : {host}:{port}"),
        Step::HttpRequest { method, url, .. } => format!("HTTP {method} : {url}"),
        Step::D6Brightness { mode, value } => format!("Luminosité D6 : {mode} {value} %"),
        Step::IfWindow { title, .. } => format!("Si la fenêtre {title} est ouverte"),
        Step::IfVariable { name, value, .. } => format!("Si {name} vaut {value}"),
        Step::SetVariable { name, value } => format!("Mémoriser {name} = {value}"),
        Step::WaitWindow { title, .. } => format!("Attendre la fenêtre {title}"),
        _ => step.name().to_owned(),
    }
}
#[cfg(test)]
mod workshop_summary_tests {
    use super::*;
    #[test]
    fn fifine_search_aliases_find_existing_actions() {
        for (kind, query) in [(0, "Super Hotkeys"), (2, "OpenApps"), (3, "Website"), (4, "SystemVolume"), (15, "Multi Action")] {
            assert!(matches_palette(kind, "Système", query));
        }
        assert!(!matches_palette(2, "Système", "OBS scene"));
    }
    #[test]
    fn summaries_do_not_repeat_the_block_name_and_wrappers_keep_the_summary() {
        let step = Step::Delay { ms: 250 };
        assert_eq!(block_summary(&step), "Attendre 250 ms");
        assert_eq!(block_summary(&Step::Disabled { step: Box::new(step) }), "Attendre 250 ms");
        assert_eq!(block_summary(&Step::SetVariable { name: "mode".into(), value: "travail".into() }), "Mémoriser mode = travail");
    }
}
const TEMPLATE_NAMES: &[&str] = &[
    "Ouvrir une application puis un site", "Copier puis coller ailleurs",
    "Taper une signature", "Basculer casque / enceintes",
    "Mode travail", "Répéter une touche N fois",
];
fn template(index: usize) -> Vec<Step> {
    match index {
        0 => vec![Step::Launch { program: String::new(), args: vec![] }, Step::Url { url: String::new() }],
        1 => vec![Step::Hotkey { keys: "CTRL+C".into() }, Step::Hotkey { keys: "CTRL+V".into() }],
        2 => vec![Step::Text { text: String::new() }],
        3 => vec![crate::automation::step(13)],
        4 => vec![Step::SetVariable { name: "mode".into(), value: "travail".into() },
            Step::IfVariable { name: "mode".into(), value: "travail".into(),
                then_steps: vec![Step::Text { text: String::new() }], else_steps: vec![] }],
        5 => vec![Step::Repeat { count: 3, steps: vec![Step::Hotkey { keys: "F13".into() }] }],
        _ => unreachable!(),
    }
}
fn template_menu(ui: &mut egui::Ui, choice: &mut Option<usize>) {
    ui.menu_button("Modèles", |ui| {
        for (i, name) in TEMPLATE_NAMES.iter().enumerate() {
            if ui.button(*name).clicked() { *choice = Some(i); ui.close_menu(); }
        }
    });
}
#[derive(Clone)]
struct TraceLine { path: Vec<usize>, text: String }
// Reflects engine::run_steps dry mode: both conditional branches and one loop
// iteration are previewed; it does not evaluate the window or invoke actions.
fn simulation_trace(steps: &[Step]) -> Vec<TraceLine> {
    fn walk(steps: &[Step], parent: &[usize], out: &mut Vec<TraceLine>, limit: usize) {
        for (i, step) in steps.iter().enumerate() {
            if out.len() >= limit { return; }
            let mut path = parent.to_vec(); path.push(i);
            let prefix = path.iter().step_by(2).map(|n| (n + 1).to_string()).collect::<Vec<_>>().join(".");
            if step.is_disabled() {
                out.push(TraceLine { path, text: format!("{prefix} · {} — désactivé, sauté", block_summary(step)) });
                continue;
            }
            let detail = match step {
                Step::Delay { ms } => format!("{ms} ms simulées"),
                Step::WaitWindow { timeout_ms, .. } => format!("jusqu'à {timeout_ms} ms (sans attendre)"),
                Step::IfWindow { .. } => "ALORS et SINON explorés (fenêtre non évaluée)".into(),
                Step::IfVariable { .. } => "ALORS et SINON explorés (valeur non évaluée)".into(),
                Step::Repeat { count, .. } => format!("1 itération simulée sur {count} prévues"),
                Step::Toggle { .. } => "États 1 et 2 explorés sans effet".into(),
                _ => "0 ms (action non exécutée)".into(),
            };
            out.push(TraceLine { path: path.clone(), text: format!("{prefix} · {} — {detail}", block_summary(step)) });
            match step {
                Step::IfWindow { then_steps, else_steps, .. } | Step::IfVariable { then_steps, else_steps, .. } => {
                    let mut p = path.clone(); p.push(0); walk(then_steps, &p, out, limit);
                    p.pop(); p.push(1); walk(else_steps, &p, out, limit);
                }
                Step::Repeat { steps, .. } => { let mut p = path; p.push(0); walk(steps, &p, out, limit); }
                Step::Toggle { a, b } => { let mut p = path.clone(); p.push(0); walk(a, &p, out, limit); p.pop(); p.push(1); walk(b, &p, out, limit); }
                _ => {}
            }
        }
    }
    let mut result = vec![];
    walk(steps, &[], &mut result, 1000);
    result
}
fn new_block(kind: usize, profiles: &[String]) -> Step {
    catalog::action(kind, profiles.first().map(String::as_str).unwrap_or("Bureau")).steps.remove(0)
}
#[allow(clippy::too_many_arguments)] // Recursive editor forwards selection and async dialog context.
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
    tx: &Sender<Option<DialogEffect>>,
    target: &(usize,String,usize),
    config: &crate::model::Config,
    advanced: bool,
) -> bool {
    let p = theme::palette(ui.ctx());
    let mut changed = false;
    let mut remove = None;
    let mut movement = None;
    let mut insertion = None;
    let mut duplicate = None;
    let mut toggle = None;
    for (i, step) in steps.iter_mut().enumerate() {
        let mut path = parent.to_vec();
        path.push(i);
        ui.push_id(i, |ui| {
            if i > 0 {
                ui.menu_button("+ Insérer ici", |ui| {
                    for &(category, kinds) in BLOCK_CATEGORIES {
                        ui.menu_button(category, |ui| {
                            for &kind in kinds {
                                if depth > 0 && (7..=11).contains(&kind) || !advanced && kind == 18 { continue; }
                                if ui.button(crate::automation::NAMES[kind]).on_hover_text(block_help(kind)).clicked() {
                                    insertion = Some((i, kind));
                                    ui.close_menu();
                                }
                            }
                        });
                    }
                });
            }
            let disabled = step.is_disabled();
            let color = if matches!(step.active(), Step::IfWindow { .. } | Step::IfVariable { .. }) {
                Color32::from_rgb(164, 123, 239)
            } else if matches!(step.active(), Step::Repeat { .. }) {
                Color32::from_rgb(239, 169, 83)
            } else {
                BLUE
            };
            let error = crate::model::validate_key(&Key { steps: vec![step.active().clone()], ..Key::default() }, config).err();
            egui::Frame::new()
                .fill(if disabled { p.surface } else { p.bg })
                .stroke(egui::Stroke::new(
                    if running == path { 2.0_f32 } else { 1.0_f32 },
                    if running == path { GREEN } else if error.is_some() && !disabled { Color32::from_rgb(255, 170, 70) } else { p.line },
                ))
                .corner_radius(12)
                .inner_margin(14)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (bar, _) = ui.allocate_exact_size(egui::vec2(4.0, 20.0), egui::Sense::hover()); ui.painter().rect_filled(bar, 2.0, color);
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
                        let folded_id = egui::Id::new("block_fold").with(&path);
                        let folded = ui.data(|d| d.get_temp::<bool>(folded_id)).unwrap_or(false);
                        if ui.selectable_label(
                            !folded,
                            if block_summary(step) == step.name() { step.name().to_owned() } else { block_summary(step) },
                        ).clicked() {
                            ui.data_mut(|d| d.insert_temp(folded_id, !folded));
                            if parent.is_empty() {
                                ui.data_mut(|d| d.insert_temp(egui::Id::new("workshop_selected"), i));
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Retirer").clicked() {
                                remove = Some(i);
                            }
                            if ui.small_button(if disabled { "Activer" } else { "Désactiver" }).clicked() { toggle = Some(i); }
                            if ui.small_button("Dupliquer").clicked() {
                                duplicate = Some(i);
                            }
                            if ui.small_button("↓").clicked() {
                                movement = Some((i, i + 1));
                            }
                            if ui
                                .add_enabled(i > 0, egui::Button::new("↑").small())
                                .clicked()
                            {
                                movement = Some((i, i - 1));
                            }
                        });
                    });
                    if let Some(e) = &error { if !disabled { ui.colored_label(Color32::from_rgb(255, 170, 70), format!("À compléter : {e}")); } }
                    if !advanced { ui.label(RichText::new(block_help(match step.active() {
                        Step::Hotkey { .. } => 0, Step::Text { .. } => 1, Step::Launch { .. } => 2,
                        Step::Url { .. } => 3, Step::Media { .. } => 4, Step::Delay { .. } => 5,
                        Step::WaitWindow { .. } => 6, Step::AudioSwitch { .. } => 13,
                        Step::IfWindow { .. } => 14, Step::Repeat { .. } => 15,
                        Step::Toggle { .. } => 36,
                        Step::SetVariable { .. } => 16, Step::IfVariable { .. } => 17,
                        Step::AutoHotkey { .. } => 18, _ => 7,
                    })).small()); }
                    if ui.data(|d| d.get_temp::<bool>(egui::Id::new("block_fold").with(&path))).unwrap_or(false) {
                        return;
                    }
                    ui.add_space(6.0);
                    let mut capture = false;
                    let mut icon = None;
                    if simple_fields_visible(step.active(), advanced) {
                        match step.active_mut() {
                            Step::Launch { program, .. } if !advanced => {
                                ui.label("Programme");
                                changed |= ui.text_edit_singleline(program).changed();
                            }
                            Step::WaitWindow { title, .. } if !advanced => {
                                ui.label("Titre de la fenêtre");
                                changed |= ui.text_edit_singleline(title).changed();
                            }
                            _ => {
                                changed |= edit_step(ui, step.active_mut(), profiles, pages, &mut capture, audio, &mut icon, tx, &(target.0,target.1.clone(),target.2,path.clone()));
                            }
                        }
                    } else { ui.label("Paramètres AutoHotkey disponibles en mode Avancé."); }
                    if let Some(state) = icon {
                        *pick_audio = Some((path.clone(), state));
                    }
                    if capture {
                        *record = Some(path.clone());
                    }
                    match step.active_mut() {
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
                                        tx,
                                        target,
                                        config,
                                        advanced,
                                    );
                                });
                            }
                        }
                        Step::Toggle { a, b } => {
                            for (branch, label, list) in [(0, "ÉTAT 1", a), (1, "ÉTAT 2", b)] {
                                ui.label(label);
                                let mut child = path.clone(); child.push(branch);
                                ui.indent(branch, |ui| { changed |= block_editor(ui, list, profiles, pages, audio, &child, record, depth + 1, running, pick_audio, tx, target, config, advanced); });
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
                                    tx,
                                    target,
                                    config,
                                    advanced,
                                );
                            });
                        }
                        _ => {}
                    }
                });
        });
    }
    if let Some(i) = toggle {
        let old = steps.remove(i);
        steps.insert(i, match old { Step::Disabled { step } => *step, step => Step::Disabled { step: Box::new(step) } });
        changed = true;
    }
    if let Some(i) = duplicate {
        if steps.len() < 256 {
            steps.insert(i + 1, steps[i].clone());
            changed = true;
        }
    }
    if let Some((i, kind)) = insertion {
        if steps.len() < 256 {
            steps.insert(i, new_block(kind, profiles));
            changed = true;
        }
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
                if i == 12 || depth > 0 && (7..=11).contains(&i) || !advanced && i == 18 {
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
        crate::obs_monitor::present_key(&crate::audio::present_key(&self.config.keys()[index], &self.audio), &self.obs_state)
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
            let viewport = ctx.input(|i| i.viewport().clone());
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            if viewport.minimized == Some(true) { ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false)); }
            if viewport.focused != Some(true) { ctx.send_viewport_cmd(egui::ViewportCommand::Focus); }
        }
        if self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.asset_browser(ctx);
        self.desktop_settings(ctx);
    }
    pub(super) fn workflow(&mut self, ctx: &egui::Context) {
        if !self.show_workflow {
            return;
        }
        let mut record = None;
        let mut pick_audio = None;
        let mut changed = false;
        let mut export = false;
        let pending_id = egui::Id::new("workshop_template_pending");
        let mut template_choice = ctx.data(|d| d.get_temp::<usize>(pending_id));
        let mut template_action = None;
        let selected_id = egui::Id::new("workshop_trace_selected");
        let trace_id = egui::Id::new("workshop_trace");
        // Text inputs must retain Delete, Alt+arrows and Ctrl+D.
        if !ctx.wants_keyboard_input() {
            let index = ctx.data(|d| d.get_temp::<usize>(egui::Id::new("workshop_selected")));
            if let Some(i) = index {
                let steps = &mut self.config.keys_mut()[self.selected].steps;
                if i < steps.len() {
                    let operation = ctx.input(|input| {
                        if input.modifiers.ctrl && input.key_pressed(egui::Key::D) { Some(0) }
                        else if !input.modifiers.ctrl && input.key_pressed(egui::Key::Delete) { Some(1) }
                        else if input.modifiers.alt && input.key_pressed(egui::Key::ArrowUp) { Some(2) }
                        else if input.modifiers.alt && input.key_pressed(egui::Key::ArrowDown) { Some(3) }
                        else { None }
                    });
                    match operation {
                        Some(0) if steps.len() < 256 => { steps.insert(i + 1, steps[i].clone()); changed = true; },
                        Some(1) => { steps.remove(i); changed = true; },
                        Some(2) if i > 0 => { steps.swap(i, i - 1); changed = true; },
                        Some(3) if i + 1 < steps.len() => { steps.swap(i, i + 1); changed = true; },
                        _ => {}
                    }
                }
            }
        }
        let title = format!(
            "Atelier · touche {:02} · {}",
            self.selected + 1,
            self.config.keys()[self.selected].label
        );
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button(format!("< Retour au deck · {title}")).clicked() {
                self.show_workflow = false;
            }
            ui.label(RichText::new("Construis ton automatisation").size(23.0).strong());
            ui.label("Assemble les blocs de haut en bas. Les variables sont propres à chaque exécution.");
            ui.horizontal_wrapped(|ui| {
                ui.label("Mode :");
                if ui.selectable_value(&mut self.preferences.workshop_advanced, false, "Simple").changed() { self.save_preferences(); }
                ui.label("|");
                if ui.selectable_value(&mut self.preferences.workshop_advanced, true, "Avancé").changed() { self.save_preferences(); }
                template_menu(ui, &mut template_choice);
                if ui.button("Créer avec mon IA").clicked() { self.assistant_settings = true; }
                if ui.add(egui::Button::new("Simuler les blocs").fill(BLUE)).clicked() {
                    if validate_key(&self.config.keys()[self.selected], &self.config).is_ok() {
                        let trace = simulation_trace(&self.config.keys()[self.selected].steps);
                        ctx.data_mut(|d| d.insert_temp(trace_id, trace.iter().map(|line| (line.path.clone(), line.text.clone())).collect::<Vec<_>>()));
                    } else { ctx.data_mut(|d| d.remove::<Vec<(Vec<usize>, String)>>(trace_id)); }
                    self.preview(true);
                }
                if ui.button("Tester dans 3 s").on_hover_text("Exécute réellement la macro sur le PC dans 3 secondes").clicked() { self.preview(false); }
                if self.preferences.workshop_advanced && ui.button("Exporter AHK").clicked() { export = true; }
            });
            if let Some(index) = template_choice {
                let existing = self.config.keys()[self.selected].steps.len();
                if existing == 0 { template_action = Some((index, false)); }
                else { ui.label("Cette touche contient déjà des blocs. Choisis comment utiliser le modèle :");
                    ui.horizontal(|ui| {
                        if ui.button("Ajouter à la fin").clicked() { template_action = Some((index, false)); }
                        if ui.button("Remplacer les blocs").clicked() { template_action = Some((index, true)); }
                    });
                }
            }
            if !self.engine.running.load(Ordering::SeqCst){self.running_block.clear();}
            let validation_config = self.config.clone();
            let profiles=self.config.profiles.iter().map(|p|p.name.clone()).collect::<Vec<_>>();let pages=self.config.profiles[self.config.active].pages.clone();let selected=self.selected;let target=(self.config.active,self.config.page.clone(),selected);let dialog_tx=self.dialog_tx.clone();
            let search_id = egui::Id::new("workshop_search");
            let mut search = ui.data(|d| d.get_temp::<String>(search_id)).unwrap_or_default();
            let mut add = None;
            let selected_trace = ctx.data(|d| d.get_temp::<Vec<usize>>(selected_id)).unwrap_or_default();
            let highlight = if selected_trace.is_empty() { &self.running_block } else { &selected_trace };
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(205.0);
                    ui.label(RichText::new("BIBLIOTHÈQUE DE BLOCS").strong());
                    egui::ScrollArea::vertical().id_salt("block_palette").show(ui, |ui| {
                        add = palette(ui, &mut search, 0, self.preferences.workshop_advanced);
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical().id_salt("workflow_scroll").show(ui,|ui|{ ui.vertical(|ui| {
                    changed=block_editor(ui,&mut self.config.keys_mut()[selected].steps,&profiles,&pages,&self.audio,&[],&mut record,0,highlight,&mut pick_audio,&dialog_tx,&target,&validation_config,self.preferences.workshop_advanced);
                }); });
            });
            ui.data_mut(|d| d.insert_temp(search_id, search));
            if let Some(kind) = add {
                let position = ui.data(|d| d.get_temp::<usize>(egui::Id::new("workshop_selected")));
                let steps = &mut self.config.keys_mut()[selected].steps;
                if steps.len() < 256 {
                    steps.insert(position.map_or(steps.len(), |i| (i + 1).min(steps.len())), new_block(kind, &profiles));
                    changed = true;
                }
            }
            if let Some(trace) = ctx.data(|d| d.get_temp::<Vec<(Vec<usize>, String)>>(trace_id)) {
                ui.separator();
                ui.label(RichText::new("Déroulé de la simulation").strong());
                ui.label("Aucune action réelle. Les deux branches sont explorées et les répétitions parcourues une seule fois.");
                egui::ScrollArea::vertical().max_height(165.0).show(ui, |ui| {
                    for (path, label) in trace {
                        if ui.selectable_label(selected_trace == path, label).clicked() {
                            ctx.data_mut(|d| d.insert_temp(selected_id, path));
                        }
                    }
                });
            }
            if let Err(e)=validate_key(&self.config.keys()[selected],&self.config){ui.colored_label(Color32::LIGHT_RED,format!("Brouillon : {e}"));}
        });
        if let Some((index, replace)) = template_action {
            let steps = &mut self.config.keys_mut()[self.selected].steps;
            let model = template(index);
            if replace { steps.clear(); }
            if steps.len() + model.len() <= 256 { steps.extend(model); changed = true; }
            template_choice = None;
        }
        ctx.data_mut(|d| { if let Some(index) = template_choice { d.insert_temp(pending_id, index); } else { d.remove::<usize>(pending_id); } });
        if let Some(target) = pick_audio {
            self.open_icon_gallery(Some(target));
        }
        if changed {
            ctx.data_mut(|d| d.remove::<Vec<(Vec<usize>, String)>>(trace_id));
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
                    let tx=self.dialog_tx.clone();
                    std::thread::spawn(move || {
                        let Some(path)=rfd::FileDialog::new().set_file_name("automatisation.ahk")
                            .add_filter("Script AutoHotkey v2", &["ahk"]).save_file() else {return};
                        let result=std::fs::write(path,text);
                        let _=tx.send(Some(Box::new(move |app: &mut App| match result {
                            Ok(())=>app.log("Script AutoHotkey exporté.".into()),
                            Err(e)=>app.fail(e.to_string()),
                        }) as DialogEffect));
                    });
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
            ui.separator();ui.label("OBS Studio (WebSocket local)");
            changed |= ui.text_edit_singleline(&mut self.config.obs.host).changed();
            changed |= ui.add(egui::DragValue::new(&mut self.config.obs.port).range(1..=65535)).changed();
            changed |= ui.add(egui::TextEdit::singleline(&mut self.config.obs.password).password(true).hint_text("Mot de passe OBS")).changed();
            ui.label(crate::obs_monitor::snapshot().status);
            if ui.button("Tester la connexion").clicked() { crate::obs_monitor::update(&self.config.obs, true); }
            ui.separator();ui.label(RichText::new(format!("Raccourci global · touche {:02}",self.selected+1)).strong());
            ui.label("Actif sur la page actuellement ouverte. Les raccourcis restent disponibles sans D6 branché.");
            let name=self.config.profiles[self.config.active].name.clone();let page=self.config.page.clone();let key=self.selected;
            let index=self.preferences.triggers.iter().position(|t|t.profile==name&&t.page==page&&t.key==key);
            let mut text=index.map(|i|self.preferences.triggers[i].keys.clone()).unwrap_or_default();
            ui.horizontal(|ui|{if ui.text_edit_singleline(&mut text).changed(){hotkeys=true;}if ui.button("Enregistrer…").clicked(){capture=true;}});
            if !text.is_empty() { match crate::desktop::chord(&text) { Ok(_) => {}, Err(e) => { ui.colored_label(Color32::LIGHT_RED,e.to_string()); } } }
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
        if changed { self.save(); if self.save_preferences() { self.refresh_hotkeys(); } }
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

#[cfg(test)]
mod workshop_tests {
    use super::*;
    #[test]
    fn trace_respects_disabled_branches_and_one_iteration_without_actions() {
        let steps = vec![Step::SetVariable { name: "mode".into(), value: "travail".into() },
            Step::IfVariable { name: "mode".into(), value: "travail".into(),
                then_steps: vec![Step::Repeat { count: 4, steps: vec![Step::Delay { ms: 250 }] }],
                else_steps: vec![Step::Disabled { step: Box::new(Step::Text { text: "secret".into() }) }] }];
        let trace = simulation_trace(&steps);
        assert_eq!(trace.len(), 5);
        assert!(trace[1].text.contains("ALORS et SINON"));
        assert!(trace[2].text.contains("1 itération simulée sur 4"));
        assert!(trace[3].text.contains("250 ms simulées"));
        assert!(trace[4].text.contains("désactivé, sauté"));
        assert_eq!(trace[3].path, vec![1, 0, 0, 0, 0]);
    }
    #[test]
    fn templates_use_existing_steps_and_become_valid_after_personalisation() {
        let c = crate::model::Config::default();
        for (i, name) in TEMPLATE_NAMES.iter().enumerate() {
            let mut steps = template(i);
            assert!(!steps.is_empty());
            fn fill(steps: &mut [Step]) {
                for s in steps {
                    assert!(simple_fields_visible(s, false), "champ du modèle inaccessible en mode Simple: {}", s.name());
                    match s {
                    Step::Launch { program, .. } => *program = "notepad.exe".into(),
                    Step::Url { url } => *url = "https://example.org".into(),
                    Step::Text { text } => *text = "Bonjour".into(),
                    Step::AudioSwitch { first, second, .. } => { *first = "device-one".into(); *second = "device-two".into(); },
                    Step::IfVariable { then_steps, else_steps, .. } => { fill(then_steps); fill(else_steps); },
                    Step::Repeat { steps, .. } => fill(steps), _ => {}
                }}
            }
            fill(&mut steps);
            assert!(crate::model::validate_key(&Key { steps, ..Key::default() }, &c).is_ok(), "{}", name);
        }
    }
    #[test]
    fn simple_mode_excludes_only_autohotkey_script_fields() {
        for kind in 0..crate::automation::NAMES.len() {
            if kind == 12 { continue; }
            let step = catalog::action(kind, "Bureau").steps.remove(0);
            assert_eq!(simple_fields_visible(&step, false), kind != 18, "{}", step.name());
            assert!(simple_fields_visible(&step, true));
        }
    }
    #[test]
    fn summaries_are_readable() {
        assert_eq!(block_summary(&Step::Delay { ms: 500 }), "Attendre 500 ms");
        assert_eq!(block_summary(&Step::Repeat { count: 3, steps: vec![Step::Back, Step::Back] }), "Répéter 3 fois (2 blocs)");
    }
}
