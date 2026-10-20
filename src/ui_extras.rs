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
        Step::IfCondition { then_steps, else_steps, .. }
        | Step::IfWindow {
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
    ("Clavier", &[0, 45]),
    ("Streaming", &[19, 20, 21, 22, 23, 24, 25, 30, 31, 0, 4]), ("Système", &[0, 2, 4, 26, 27, 28, 29, 32, 33]),
    ("Texte", &[1, 43, 44]), ("Applications et sites", &[2, 3]),
    ("Médias et audio", &[4, 13, 37, 38]), ("Fenêtres", &[6, 46]), ("Attendre", &[5, 40]), ("Notifications", &[41]),
    ("Logique et répétitions", &[14, 15, 39]), ("Variables", &[16, 17, 42]),
    ("Navigation DeckForge", &[7, 8, 9, 10, 11]), ("Avancé / AutoHotkey", &[18]),
];
fn block_help(kind: usize) -> &'static str {
    match kind {
        27 => "Clique à la position actuelle, ou à des coordonnées écran, zone client ou curseur.",
        28 => "Déplace la souris dans l’écran, depuis le curseur ou dans la zone client capturée au déclenchement.",
        29 => "Défile verticalement ou horizontalement ; simulation sans effet.",
        46 => "Active, déplace ou redimensionne une fenêtre ; jamais DeckForge sauf ciblage explicite par titre.",
        45 => "Maintient des touches ; 0 ms jusqu’au bloc Relâcher, fin ou annulation de la macro.",
        0 => "Appuie sur une combinaison de touches.", 1 => "Écrit du texte dans la fenêtre active ; accepte {nom} et les variables spéciales.",
        2 => "Démarre un programme Windows (arguments interpolés).", 3 => "Ouvre une adresse web (gabarit interpolé).",
        4 => "Commande la lecture et le volume.", 5 => "Attend avant le bloc suivant.",
        6 => "Attend et peut activer une fenêtre.", 13 => "Bascule entre deux appareils audio.", 37 => "Règle uniquement les sessions audio de l’application ciblée.", 38 => "Règle le volume du périphérique (sortie ou microphone), et non celui d’une application.",
        39 => "Évalue une condition en lecture seule ; exécute ALORS ou SINON.",
        40 => "Attend une condition Windows en lecture seule, avec délai et bouton Arrêter.",
        41 => "Affiche un message Windows sans voler le focus ; accepte les variables de macro." ,
        14 => "Exécute des blocs si une fenêtre est ouverte.", 15 => "Rejoue plusieurs fois les blocs imbriqués.",
        42 => "Calcule une valeur numérique dans une variable de la macro.",
        43 => "Transforme une variable texte pendant cette macro, sans saisir au clavier.",
        44 => "Copie, lit, vide, sauvegarde ou restaure le texte Unicode du presse-papiers.",
        16 => "Enregistre une valeur pour cette exécution.", 17 => "Choisit des blocs selon une valeur.",
        19..=25 => "Commande OBS Studio par WebSocket local (OBS 28 ou plus).",
        32 => "Règle ou ajuste la luminosité du périphérique (0 à 100 %).",
        33 => "Met le périphérique en veille ; le prochain appui le réveille.",
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
fn palette(ui: &mut egui::Ui, search: &str, depth: usize, advanced: bool, keyboard: Option<usize>) -> Option<usize> {
    let mut selected = None;
    let mut found = false;
    let mut index = 0;
    for &(category, kinds) in BLOCK_CATEGORIES {
        let matches: Vec<_> = kinds.iter().copied().filter(|&kind| {
            (depth == 0 || !(7..=11).contains(&kind)) &&
            (advanced || kind != 18) && matches_palette(kind, category, search)
        }).collect();
        if matches.is_empty() { continue; }
        found = true;
        egui::CollapsingHeader::new(category).default_open(search.is_empty() && category == "Clavier" || !search.is_empty())
            .show(ui, |ui| {
                for kind in matches {
                    ui.horizontal(|ui| {
                        let color = if category.contains("Logique") { Color32::from_rgb(164, 123, 239) }
                            else if category.contains("Streaming") { Color32::from_rgb(239, 169, 83) } else { accent() };
                        family_bar(ui, color);
                        let label = if keyboard == Some(index) { format!("> {}", crate::automation::NAMES[kind]) }
                            else { crate::automation::NAMES[kind].to_owned() };
                        if ui.add(egui::Button::new(label).frame(false))
                            .on_hover_text(block_help(kind)).clicked() { selected = Some(kind); }
                        index += 1;
                    });
                }
            });
    }
    if !found { ui.label("Aucun bloc trouvé. Essaie un autre mot-clé."); }
    selected
}
fn simple_fields_visible(step: &Step, advanced: bool) -> bool {
    advanced || !matches!(step.active(), Step::AutoHotkey { .. })
}
pub(super) fn block_summary(step: &Step) -> String {
    match step.active() {
        Step::MouseClick { .. } | Step::MouseMove { .. } | Step::MouseScroll { .. } => super::step_summary(step),
        Step::Clipboard { .. } => step.clipboard_summary().unwrap_or_default(),
        Step::WindowOp { .. } => step.window_op_summary().unwrap_or_default(),
        Step::AppVolume { .. } => step.app_volume_summary().unwrap_or_default(),
        Step::DeviceVolume { .. } => step.device_volume_summary().unwrap_or_default(),
        Step::ObsScene { name } => format!("OBS : scène {name}"),
        Step::ObsRecord { mode } | Step::ObsStream { mode } | Step::ObsVirtualCam { mode } | Step::ObsReplay { mode } => format!("{} : {mode}", step.name()),
        Step::ObsMute { input, mode } => format!("OBS : {input} ({mode})"),
        Step::ObsSourceVisibility { scene, source, mode } => format!("OBS : {scene} / {source} ({mode})"),
        Step::Hotkey { keys } => format!("Appuyer sur {keys}"),
        Step::KeyHold { .. } => step.key_hold_summary().unwrap_or_default(),
        Step::Delay { ms } => format!("Attendre {ms} ms"),
        Step::Url { url } => format!("Ouvrir {url}"),
        Step::Text { text } => format!("Écrire {}", text.chars().take(50).collect::<String>()),
        Step::Launch { program, .. } => format!("Lancer {program}"),
        Step::Toggle { a, b } => format!("Basculer : {} / {} blocs", a.len(), b.len()),
        Step::Repeat { count, steps } => format!("Répéter {count} fois ({} blocs)", steps.len()),
        Step::UdpSend { host, port, .. } => format!("UDP : {host}:{port}"),
        Step::HttpRequest { method, url, .. } => format!("HTTP {method} : {url}"),
        Step::D6Brightness { mode, value } => format!("Luminosité : {mode} {value} %"),
        Step::IfCondition { condition, .. } => format!("Si {}", condition.summary()),
        Step::WaitUntil { condition, timeout_ms, .. } => format!("Attendre que {} ({} ms max)", condition.summary(), timeout_ms),
        Step::Notify { title, message } => format!("Notification « {title} » : {}", message.chars().take(50).collect::<String>()),
        Step::IfWindow { title, .. } => format!("Si la fenêtre {title} est ouverte"),
        Step::IfVariable { name, value, .. } => format!("Si {name} vaut {value}"),
        Step::Math { .. } => step.math_summary().unwrap_or_default(),
        Step::TextOp { .. } => step.text_op_summary().unwrap_or_default(),
        Step::SetVariable { name, value, persistent } => format!("Mémoriser {name} = {value}{}", if *persistent { " (enregistrée)" } else { "" }),
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
        assert_eq!(block_summary(&Step::SetVariable { name: "mode".into(), value: "travail".into(), persistent: false, }), "Mémoriser mode = travail");
    }
}
const TEMPLATE_NAMES: &[&str] = &[
    "Ouvrir une application puis un site", "Copier puis coller ailleurs",
    "Taper une signature", "Basculer casque / enceintes",
    "Mode travail", "Répéter une touche N fois",
];
fn template(index: usize) -> Vec<Step> {
    match index {
        0 => vec![Step::Launch { program: String::new(), args: vec![], focus_if_running: false }, Step::Url { url: String::new() }],
        1 => vec![Step::Hotkey { keys: "CTRL+C".into() }, Step::Hotkey { keys: "CTRL+V".into() }],
        2 => vec![Step::Text { text: String::new() }],
        3 => vec![crate::automation::step(13)],
        4 => vec![Step::SetVariable { name: "mode".into(), value: "travail".into(), persistent: false, },
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
#[cfg(test)]
struct TraceLine { path: Vec<usize>, text: String }
// Reflects engine::run_steps dry mode: both conditional branches and one loop
// iteration are previewed; it does not evaluate the window or invoke actions.
#[cfg(test)]
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
                Step::WaitUntil { .. } => "condition lue une fois ; sans attendre".into(),
                Step::IfCondition { .. } => "ALORS et SINON explorés ; condition lue sans effet".into(),
                Step::IfWindow { .. } => "ALORS et SINON explorés (fenêtre non évaluée)".into(),
                Step::IfVariable { .. } => "ALORS et SINON explorés (valeur non évaluée)".into(),
                Step::Repeat { count, .. } => format!("1 itération simulée sur {count} prévues"),
                Step::Toggle { .. } => "États 1 et 2 explorés sans effet".into(),
                _ => "0 ms (action non exécutée)".into(),
            };
            out.push(TraceLine { path: path.clone(), text: format!("{prefix} · {} — {detail}", block_summary(step)) });
            match step {
                Step::IfCondition { then_steps, else_steps, .. } | Step::IfWindow { then_steps, else_steps, .. } | Step::IfVariable { then_steps, else_steps, .. } => {
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
    let step_count = steps.len();
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
            let color = if matches!(step.active(), Step::IfWindow { .. } | Step::IfVariable { .. } | Step::IfCondition { .. }) {
                Color32::from_rgb(164, 123, 239)
            } else if matches!(step.active(), Step::Repeat { .. }) {
                Color32::from_rgb(239, 169, 83)
            } else {
                accent()
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
                        block_glyph(ui, step, color);
                        let folded_id = egui::Id::new("block_fold").with(&path);
                        let folded = ui.data(|d| d.get_temp::<bool>(folded_id)).unwrap_or(true);
                        let summary = block_summary(step);
                        if ui.selectable_label(!folded, if summary.starts_with(step.name()) { summary.clone() } else { format!("{} · {}", step.name(), summary) })
                            .on_hover_text(format!("{} — {}", step.name(), summary)).clicked() {
                            ui.data_mut(|d| d.insert_temp(folded_id, !folded));
                            if parent.is_empty() {
                                ui.data_mut(|d| d.insert_temp(egui::Id::new("workshop_selected"), i));
                            }
                        }
                        ui.menu_button("...", |ui| {
                            if ui.button("Dupliquer").clicked() { duplicate = Some(i); ui.close_menu(); }
                            if ui.button("Retirer").clicked() { remove = Some(i); ui.close_menu(); }
                            if ui.button(if disabled { "Activer" } else { "Désactiver" }).clicked() { toggle = Some(i); ui.close_menu(); }
                            if ui.add_enabled(i > 0, egui::Button::new("Monter")).clicked() { movement = Some((i, i - 1)); ui.close_menu(); }
                            if ui.add_enabled(i + 1 < step_count, egui::Button::new("Descendre")).clicked() { movement = Some((i, i + 1)); ui.close_menu(); }
                        });
                    });
                    if let Some(e) = &error { if !disabled { ui.colored_label(Color32::from_rgb(255, 170, 70), format!("À compléter : {e}")); } }
                    if !advanced { ui.label(RichText::new(block_help(match step.active() {
                        Step::Hotkey { .. } => 0, Step::Text { .. } => 1, Step::Launch { .. } => 2,
                        Step::Url { .. } => 3, Step::Media { .. } => 4, Step::Delay { .. } => 5,
                        Step::WaitWindow { .. } => 6, Step::AudioSwitch { .. } => 13, Step::DeviceVolume { .. } => 38,
                        Step::IfWindow { .. } => 14, Step::IfCondition { .. } => 39, Step::WaitUntil { .. } => 40, Step::Notify { .. } => 41, Step::Repeat { .. } => 15,
                        Step::Toggle { .. } => 36,
                        Step::SetVariable { .. } => 16, Step::IfVariable { .. } => 17, Step::Math { .. } => 42, Step::TextOp { .. } => 43,
                        Step::AutoHotkey { .. } => 18, _ => 7,
                    })).small()); }
                    if ui.data(|d| d.get_temp::<bool>(egui::Id::new("block_fold").with(&path))).unwrap_or(true) {
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
                        Step::IfCondition { then_steps, else_steps, .. }
                        | Step::IfWindow {
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
/// Thin family colour bar (font-independent, unlike box-drawing glyphs).
fn family_bar(ui: &mut egui::Ui, color: Color32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(3.0, 14.0), egui::Sense::hover());
    ui.painter().rect_filled(r, 1.0, color);
}
// Small vector icons: no dependency on glyph coverage in the UI font.
fn block_glyph(ui: &mut egui::Ui, step: &Step, color: Color32) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
    let p = ui.painter();
    let c = r.center();
    let stroke = egui::Stroke::new(1.4_f32, color);
    match step.active() {
        Step::Delay { .. } | Step::WaitWindow { .. } | Step::WaitUntil { .. } => {
            p.circle_stroke(c, 7.0, stroke);
            p.line_segment([c, c + egui::vec2(0.0, -4.0)], stroke);
            p.line_segment([c, c + egui::vec2(4.0, 2.0)], stroke);
        }
        Step::IfCondition { .. } | Step::IfWindow { .. } | Step::IfVariable { .. } | Step::Toggle { .. } => {
            for dy in [-5.0, 5.0] { p.line_segment([c - egui::vec2(6.0, 0.0), c + egui::vec2(6.0, dy)], stroke); }
        }
        Step::Media { .. } | Step::AppVolume { .. } | Step::DeviceVolume { .. } => {
            for (x, h) in [(-5.0, 3.0), (0.0, 6.0), (5.0, 4.0)] {
                p.line_segment([c + egui::vec2(x, -h), c + egui::vec2(x, h)], stroke);
            }
        }
        _ => {
            p.rect_filled(r.shrink(2.0), 3.0, color.gamma_multiply(0.25));
            p.line_segment([c - egui::vec2(4.0, 0.0), c + egui::vec2(4.0, 0.0)], stroke);
        }
    }
}
// A compact card tree; structural edits are applied only after the immutable render pass.
fn sequence_cards(ui: &mut egui::Ui, steps: &[Step], parent: &[usize], selection: &mut Option<Vec<usize>>,
    action: &mut Option<WorkshopAction>, insert_at: &mut Option<Vec<usize>>, config: &crate::model::Config,
    ) {
    let p = theme::palette(ui.ctx());
    for (i, step) in steps.iter().enumerate() {
        let mut path = parent.to_vec(); path.push(i);
        ui.push_id(&path, |ui| {
            let chosen = selection.as_ref() == Some(&path);
            if i > 0 {
                let line = ui.allocate_response(egui::vec2(ui.available_width().min(520.0).max(120.0), 6.0), egui::Sense::hover());
                ui.painter().line_segment([egui::pos2(line.rect.center().x, line.rect.top()),
                    egui::pos2(line.rect.center().x, line.rect.bottom())], egui::Stroke::new(1.0_f32, p.line));
                if line.hovered() {
                    let plus = egui::Rect::from_center_size(line.rect.center(), egui::vec2(22.0, 16.0));
                    if ui.put(plus, egui::Button::new("+").small()).clicked() { *insert_at = Some(path.clone()); }
                }
            }
            let error = if step.is_disabled() { None } else {
                crate::model::validate_key(&Key { steps: vec![step.clone()], ..Key::default() }, config).err()
            };
            let frame = egui::Frame::new().fill(p.surface)
                .stroke(egui::Stroke::new(if chosen { 1.5_f32 } else { 1.0_f32 }, if chosen { accent() } else { p.line }))
                .corner_radius(10).inner_margin(9).show(ui, |ui| {
                    ui.set_width((ui.available_width() - 20.0).clamp(120.0, 520.0));
                    let color = if matches!(step.active(), Step::IfCondition { .. } | Step::IfWindow { .. } | Step::IfVariable { .. }) {
                        Color32::from_rgb(164, 123, 239)
                    } else if matches!(step.active(), Step::Repeat { .. }) { Color32::from_rgb(239, 169, 83) } else { accent() };
                    ui.horizontal(|ui| {
                        block_glyph(ui, step, color);
                        ui.colored_label(color, path.iter().step_by(2).map(|n| (n + 1).to_string()).collect::<Vec<_>>().join("."));
                        let summary = block_summary(step);
                        let title = if summary.starts_with(step.name()) { summary.clone() } else { format!("{} · {}", step.name(), summary) };
                        let label = ui.add(egui::Button::new(egui::RichText::new(title)).selected(chosen)
                            .truncate().sense(egui::Sense::click_and_drag()))
                            .on_hover_text(format!("{} : {}. Cliquer pour afficher les propriétés ; glisser pour déplacer.", step.name(), summary));
                        if label.clicked() { *selection = if chosen { None } else { Some(path.clone()) }; }
                        if label.drag_started() { ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("workshop_drag"), path.clone())); }
                        if let Some(e) = &error { ui.colored_label(Color32::LIGHT_RED, "Erreur").on_hover_text(e.to_string()); }
                        ui.menu_button("...", |ui| {
                            if ui.button("Dupliquer").clicked() { *action = Some(WorkshopAction::Edit(crate::workshop_ops::Edit::Duplicate(path.clone()))); ui.close_menu(); }
                            if ui.button("Supprimer").clicked() { *action = Some(WorkshopAction::Remove(path.clone())); ui.close_menu(); }
                            if ui.add_enabled(i > 0, egui::Button::new("Monter")).clicked() {
                                *action = Some(WorkshopAction::Edit(crate::workshop_ops::Edit::Move(path.clone(), i - 1))); ui.close_menu();
                            }
                            if ui.add_enabled(i + 1 < steps.len(), egui::Button::new("Descendre")).clicked() {
                                *action = Some(WorkshopAction::Edit(crate::workshop_ops::Edit::Move(path.clone(), i + 1))); ui.close_menu();
                            }
                            if ui.button(if step.is_disabled() { "Réactiver" } else { "Désactiver" }).clicked() {
                                *action = Some(WorkshopAction::Edit(crate::workshop_ops::Edit::Toggle(path.clone()))); ui.close_menu();
                            }
                            if ui.button("Insérer un bloc après").clicked() {
                                let mut dest = path.clone(); *dest.last_mut().unwrap() += 1;
                                *insert_at = Some(dest); ui.close_menu();
                            }
                        });
                    });
                    let branches: Vec<(&str, &[Step])> = match step.active() {
                        Step::IfCondition { then_steps, else_steps, .. }
                        | Step::IfWindow { then_steps, else_steps, .. }
                        | Step::IfVariable { then_steps, else_steps, .. } => vec![("ALORS", then_steps), ("SINON", else_steps)],
                        Step::Repeat { steps, .. } => vec![("RÉPÉTER", steps)],
                        Step::Toggle { a, b } => vec![("ÉTAT 1", a), ("ÉTAT 2", b)],
                        _ => vec![],
                    };
                    for (branch, (label, children)) in branches.into_iter().enumerate() {
                        ui.indent(branch, |ui| {
                            ui.label(RichText::new(label).small().color(p.muted));
                            let mut nested = path.clone(); nested.push(branch);
                            sequence_cards(ui, children, &nested, selection, action, insert_at, config);
                        });
                    }
                });
            if frame.response.hovered() && ui.ctx().input(|input| input.pointer.any_released()) {
                if let Some(from) = ui.ctx().data(|d| d.get_temp::<Vec<usize>>(egui::Id::new("workshop_drag"))) {
                    if from != path && from[..from.len() - 1] == path[..path.len() - 1] {
                        let dest = i;
                        *action = Some(WorkshopAction::Edit(crate::workshop_ops::Edit::Move(from, dest)));
                    }
                }
            }
            if ui.ctx().data(|d| d.get_temp::<Vec<usize>>(egui::Id::new("workshop_drag"))).is_some()
                && frame.response.hovered() && ui.ctx().input(|input| input.pointer.primary_down()) {
                let r = frame.response.rect;
                let y = if ui.ctx().input(|input| input.pointer.hover_pos()).is_some_and(|pos| pos.y > r.center().y) { r.bottom() } else { r.top() };
                ui.painter().line_segment([egui::pos2(r.left(), y), egui::pos2(r.right(), y)], egui::Stroke::new(2.0_f32, accent()));
            }
        });
    }
}
enum WorkshopAction { Edit(crate::workshop_ops::Edit), Remove(Vec<usize>) }
impl App {
    pub(super) fn automation_prompt(&self) -> String {
        let profile = &self.config.profiles[self.config.active].name;
        let destination =
            serde_json::json!({"profile":profile,"page":self.config.page,"key":self.selected+1});
        format!("Utilise le connecteur MCP DeckForge pour proposer cette automatisation :\n{}\n\nDestination exacte : {destination}\nLis d’abord deckforge_get_state et deckforge_get_profile. Si la demande concerne l’audio, utilise deckforge_get_audio_devices pour récupérer les identifiants réels. Tu peux utiliser deckforge_get_icons ; pour audio_switch, first_icon/second_icon acceptent builtin:headphones, builtin:speakers et builtin:microphone. Crée les étapes avec deckforge_configure_key (conditions, répétitions et variables possibles). Si une information manque, demande-la-moi au lieu d’inventer un périphérique, un programme ou un script. N’exécute aucune macro. Attends la validation de la proposition dans DeckForge ; les blocs resteront ensuite modifiables dans l’Atelier.",self.assistant_request.trim())
    }
    pub(super) fn presented_key(&self, index: usize) -> Key {
        self.state_key(index, crate::obs_monitor::present_key(&crate::audio::present_key(&self.config.keys()[index], &self.audio), &self.obs_state))
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
                .filter(|t| t.kind == "hotkey" && &t.profile == name && t.page == self.config.page && t.key < 15)
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
        self.saved_variables_window(ctx);
    }
    fn saved_variables_window(&mut self, ctx: &egui::Context) {
        if !self.show_saved_variables { return; }
        let mut open = self.show_saved_variables;
        egui::Window::new("Variables enregistrées").open(&mut open).resizable(true).show(ctx, |ui| {
            ui.label("Conservées dans variables.json ; {nom} les utilise dans les macros suivantes.");
            let mut remove = None;
            for (name, original) in crate::persistent_variables::snapshot() {
                ui.horizontal(|ui| {
                    ui.label(format!("{name} :"));
                    let mut value = original.clone();
                    if ui.text_edit_singleline(&mut value).changed() && value != original {
                        if let Err(e) = crate::persistent_variables::set(&name, &value) {
                            self.fail(format!("Variable non enregistrée : {e}"));
                        }
                    }
                    if ui.add(theme::danger_button("Supprimer")).clicked() { remove = Some(name); }
                });
            }
            if let Some(name) = remove {
                if let Err(e) = crate::persistent_variables::remove(&name) {
                    self.fail(format!("Variable non supprimée : {e}"));
                }
            }
        });
        self.show_saved_variables = open;
    }
    fn start_workshop_run(&mut self, dry: bool) {
        if self.engine.running.load(Ordering::SeqCst) { return; }
        let mut key = self.config.keys()[self.selected].clone();
        if key.steps.is_empty() { self.fail("Ajoute un bloc avant de tester.".into()); return; }
        if let Err(e) = validate_key(&key, &self.config) { self.fail(format!("Action à corriger : {e}")); return; }
        key.runtime_id = Some(crate::engine::identity(&self.config.profiles[self.config.active].name, &self.config.page, self.selected));
        // A workshop run never queues behind another macro; no real side effect on dry mode.
        key.run_mode = Some(crate::model::RunMode::Ignore);
        key.debounce_ms = 0;
        self.workshop_journal.begin(dry);
        self.engine.start_workshop(key, dry, self.config.obs.clone());
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
        let context_id = egui::Id::new("workshop_context");
        let current = (self.config.active, self.config.page.clone(), self.selected);
        if ctx.data(|d| d.get_temp::<(usize, String, usize)>(context_id)).as_ref() != Some(&current) {
            ctx.data_mut(|d| {
                d.insert_temp(context_id, current.clone());
                d.remove::<Vec<usize>>(egui::Id::new("workshop_path"));
                d.remove::<usize>(egui::Id::new("workshop_selected"));
                d.remove::<Vec<usize>>(egui::Id::new("workshop_insert_at"));
            });
        }
        let path_id = egui::Id::new("workshop_path");
        let mut selection = ctx.data(|d| d.get_temp::<Vec<usize>>(path_id));
        if selection.as_ref().is_some_and(|path| step_mut(&mut self.config.keys_mut()[self.selected].steps, path).is_none()) {
            selection = None;
        }
        let mut action: Option<WorkshopAction> = None;
        let mut insert_at = ctx.data(|d| d.get_temp::<Vec<usize>>(egui::Id::new("workshop_insert_at")));
        if !ctx.wants_keyboard_input() {
            if let Some(path) = selection.clone() {
                let index = *path.last().unwrap();
                let len = { let mut tree = self.config.keys()[self.selected].steps.clone();
                    let mut p = path.clone(); p.pop(); if !p.is_empty() { p.pop(); }
                    if p.is_empty() { tree.len() } else {
                        if let Some(parent) = crate::workshop_ops::get(&mut tree, &p) {
                        match (parent.active(), path[path.len() - 2]) {
                            (Step::Repeat { steps, .. }, 0) => steps.len(),
                            (Step::Toggle { a, .. }, 0) => a.len(), (Step::Toggle { b, .. }, 1) => b.len(),
                            (Step::IfCondition { then_steps, .. } | Step::IfWindow { then_steps, .. } | Step::IfVariable { then_steps, .. }, 0) => then_steps.len(),
                            (Step::IfCondition { else_steps, .. } | Step::IfWindow { else_steps, .. } | Step::IfVariable { else_steps, .. }, 1) => else_steps.len(),
                            _ => 0,
                        }
                        } else { 0 }
                    }
                };
                let op = ctx.input(|input| {
                    if input.modifiers.ctrl && input.key_pressed(egui::Key::D) { Some(0) }
                    else if !input.modifiers.ctrl && input.key_pressed(egui::Key::Delete) { Some(1) }
                    else if input.modifiers.ctrl && input.key_pressed(egui::Key::ArrowUp) { Some(2) }
                    else if input.modifiers.ctrl && input.key_pressed(egui::Key::ArrowDown) { Some(3) }
                    else if !input.modifiers.ctrl && input.key_pressed(egui::Key::ArrowUp) { Some(4) }
                    else if !input.modifiers.ctrl && input.key_pressed(egui::Key::ArrowDown) { Some(5) }
                    else if input.key_pressed(egui::Key::Escape) { Some(6) } else { None }
                });
                match op {
                    Some(0) => action = Some(WorkshopAction::Edit(crate::workshop_ops::Edit::Duplicate(path))),
                    Some(1) => action = Some(WorkshopAction::Remove(path)),
                    Some(2) if index > 0 => action = Some(WorkshopAction::Edit(crate::workshop_ops::Edit::Move(path, index - 1))),
                    Some(3) if index + 1 < len => action = Some(WorkshopAction::Edit(crate::workshop_ops::Edit::Move(path, index + 1))),
                    Some(4) if index > 0 => { *selection.as_mut().unwrap().last_mut().unwrap() -= 1; },
                    Some(5) if index + 1 < len => { *selection.as_mut().unwrap().last_mut().unwrap() += 1; },
                    Some(6) => selection = None,
                    _ => {}
                }
            }
        }
        let compact = ctx.screen_rect().width() < 1500.0;
        let palette_id = egui::Id::new("workshop_palette_open");
        let mut palette_open = ctx.data(|d| d.get_temp::<bool>(palette_id)).unwrap_or(!compact);
        if compact && self.workshop_chat.open { palette_open = false; }
        egui::TopBottomPanel::top("workshop_header").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("← Retour au deck").clicked() { self.show_workflow = false; selection = None; }
                let key = &self.config.keys()[self.selected];
                ui.label(RichText::new(format!("Atelier · Touche {:02}{}", self.selected + 1,
                    if key.label.is_empty() { String::new() } else { format!(" · {}", key.label) })).strong());
                ui.separator();
                if ui.selectable_value(&mut self.preferences.workshop_advanced, false, "Simple").changed() { self.save_preferences(); }
                if ui.selectable_value(&mut self.preferences.workshop_advanced, true, "Avancé").changed() { self.save_preferences(); }
                if compact && ui.button(if palette_open { "Masquer les blocs" } else { "Blocs" }).clicked() { palette_open = !palette_open; }
                template_menu(ui, &mut template_choice);
                if self.preferences.workshop_advanced && ui.button("Exporter AHK").clicked() { export = true; }
                if ui.add(egui::Button::new("Enregistrer").fill(accent())).clicked() { self.save(); }
            });
            if let Some(index) = template_choice {
                let existing = self.config.keys()[self.selected].steps.len();
                if existing == 0 { template_action = Some((index, false)); }
                else {
                    ui.label("Cette touche contient déjà des blocs. Choisis comment utiliser le modèle :");
                    ui.horizontal(|ui| {
                        if ui.button("Ajouter à la fin").clicked() { template_action = Some((index, false)); }
                        if ui.button("Remplacer les blocs").clicked() { template_action = Some((index, true)); }
                    });
                }
            }
        });
        let search_id = egui::Id::new("workshop_search");
        let mut search = ctx.data(|d| d.get_temp::<String>(search_id)).unwrap_or_default();
        let mut add = None;
        ctx.data_mut(|d| d.insert_temp(palette_id, palette_open));
        if (!compact || palette_open) && !(compact && self.workshop_chat.open) {
        egui::SidePanel::left("workshop_library").resizable(!compact).default_width(if compact { 240.0 } else { 280.0 }).min_width(200.0).max_width(if compact { 260.0 } else { 350.0 }).show(ctx, |ui| {
            ui.label(RichText::new("BIBLIOTHÈQUE DE BLOCS").strong());
            let response = ui.add(egui::TextEdit::singleline(&mut search).hint_text("Rechercher un bloc"));
            let kinds: Vec<_> = BLOCK_CATEGORIES.iter().flat_map(|(category, kinds)| kinds.iter().copied()
                .filter(|&kind| (self.preferences.workshop_advanced || kind != 18) && matches_palette(kind, category, &search))).collect();
            let cursor_id = egui::Id::new("workshop_palette_cursor");
            let mut cursor = ctx.data(|d| d.get_temp::<usize>(cursor_id)).unwrap_or(0);
            if !kinds.is_empty() { cursor = cursor.min(kinds.len()-1); }
            if response.changed() { cursor = 0; }
            if response.has_focus() && !kinds.is_empty() {
                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) { cursor = (cursor+1) % kinds.len(); }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) { cursor = (cursor+kinds.len()-1) % kinds.len(); }
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) { add = Some(kinds[cursor]); }
            }
            ctx.data_mut(|d| d.insert_temp(cursor_id, cursor));
            ui.separator();
            egui::ScrollArea::vertical().id_salt("block_palette").show(ui, |ui| {
                let clicked = palette(ui, &search, 0, self.preferences.workshop_advanced, response.has_focus().then_some(cursor));
                if clicked.is_some() { add = clicked; }
            });
        });
        }
        ctx.data_mut(|d| d.insert_temp(search_id, search));
        if !self.engine.running.load(Ordering::SeqCst) { self.running_block.clear(); }
        let validation_config = self.config.clone();
        let profiles = self.config.profiles.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
        let pages = self.config.profiles[self.config.active].pages.clone();
        let selected = self.selected;
        let target = (self.config.active, self.config.page.clone(), selected);
        let dialog_tx = self.dialog_tx.clone();
        let can_run = !self.engine.running.load(Ordering::SeqCst) && !self.workshop_journal.confirm_real;
        egui::TopBottomPanel::bottom("workshop_journal_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                let arrow = if self.workshop_journal.open { "Masquer" } else { "Journal" };
                if ui.button(arrow).clicked() { self.workshop_journal.open = !self.workshop_journal.open; }
                ui.add(egui::Label::new(self.workshop_journal.summary()).truncate());
                if ui.add_enabled(can_run, egui::Button::new("Simuler")).clicked() { self.start_workshop_run(true); }
                if ui.add_enabled(can_run, egui::Button::new("Tester pour de vrai")).clicked() {
                    self.workshop_journal.confirm_real = true;
                }
                if ui.button(if self.workshop_chat.open { "Fermer l’assistant" } else { "Assistant" }).clicked() { self.workshop_chat.open = !self.workshop_chat.open; }
            });
            if self.workshop_journal.open {
                ui.separator();
                ui.label(if self.workshop_journal.mode == Some(false) { "EXÉCUTION RÉELLE" } else { "SIMULATION" });
                egui::ScrollArea::vertical().id_salt("workshop_journal_lines").max_height(145.0).show(ui, |ui| {
                    for line in &self.workshop_journal.lines {
                        let title = if let Some(path) = &line.path {
                            format!("{} · {}", path.iter().step_by(2).map(|i| (i + 1).to_string()).collect::<Vec<_>>().join("."), line.message)
                        } else { line.message.clone() };
                        let title = if let Some(elapsed) = line.elapsed { format!("{title} · {:.1} ms", elapsed.as_secs_f64() * 1000.0) } else { title };
                        if let Some(path) = &line.path {
                            if ui.button(title).clicked() { selection = Some(path.clone()); }
                        } else { ui.label(title); }
                    }
                    if let Some(message) = &self.workshop_journal.error { ui.colored_label(Color32::LIGHT_RED, message); }
                });
            }
        });
        let selected_trace = ctx.data(|d| d.get_temp::<Vec<usize>>(selected_id)).unwrap_or_default();
        if !self.preferences.workshop_advanced && !self.workshop_chat.open && (!compact || !palette_open) {
            if let Some(path) = selection.clone() {
                let mut close = false;
                egui::SidePanel::right("workshop_properties_panel")
                    .resizable(!compact).default_width(if compact { 300.0 } else { 340.0 })
                    .min_width(260.0).max_width(360.0).show(ctx, |ui| {
                        if ui.small_button("× Fermer les propriétés").clicked() { close = true; }
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            if let Some(step) = step_mut(&mut self.config.keys_mut()[selected].steps, &path) {
                                ui.label(RichText::new("Propriétés du bloc").strong());
                                ui.label(RichText::new(step.name()).strong());
                                ui.label(block_summary(step));
                                 if !step.is_disabled() {
                                     if let Err(e) = validate_key(&Key { steps: vec![step.clone()], ..Key::default() }, &validation_config) {
                                         ui.colored_label(Color32::LIGHT_RED, format!("À corriger : {e}"));
                                     }
                                 }
                                let mut capture = false;
                                let mut icon = None;
                                changed |= edit_step(ui, step, &profiles, &pages, &mut capture, &self.audio, &mut icon,
                                    &dialog_tx, &(target.0, target.1.clone(), target.2, path.clone()));
                                if capture { record = Some(path.clone()); }
                                if let Some(state) = icon { pick_audio = Some((path.clone(), state)); }
                            }
                        });
                    });
                if close { selection = None; }
            }
        }
        if self.workshop_journal.confirm_real {
            egui::Window::new("Confirmer l'exécution réelle").collapsible(false).resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                    ui.label(format!("Ceci va réellement exécuter {} blocs sur ce PC.", self.config.keys()[selected].steps.len()));
                    ui.label("Les actions peuvent modifier les applications, le son et les périphériques.");
                    ui.horizontal(|ui| {
                        if ui.button("Annuler").clicked() { self.workshop_journal.confirm_real = false; }
                        if ui.add(theme::danger_button("Confirmer l'exécution réelle")).clicked() {
                            self.workshop_journal.confirm_real = false;
                            self.start_workshop_run(false);
                        }
                    });
                });
        }
        self.workshop_chat(ctx, selection.as_deref());
        let highlight = if selected_trace.is_empty() { &self.running_block } else { &selected_trace };
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(RichText::new("Construis ton automatisation").size(21.0).strong());
            if self.preferences.workshop_advanced {
                egui::ScrollArea::vertical().id_salt("workflow_advanced_scroll").show(ui, |ui| {
                    changed |= block_editor(ui, &mut self.config.keys_mut()[selected].steps, &profiles, &pages, &self.audio,
                        &[], &mut record, 0, highlight, &mut pick_audio, &dialog_tx, &target, &validation_config, true);
                });
            } else {
                egui::ScrollArea::vertical().id_salt("workflow_scroll").show(ui, |ui| {
                    let steps = &self.config.keys()[selected].steps;
                    if steps.is_empty() {
                        ui.add_space(24.0);
                        ui.label(RichText::new("Ajoute ton premier bloc").size(20.0).strong());
                        ui.label("Commence par une action, puis compose ta séquence.");
                        ui.horizontal_wrapped(|ui| {
                            for kind in [0, 2, 5, 39] {
                                if ui.button(crate::automation::NAMES[kind]).clicked() { add = Some(kind); }
                            }
                        });
                        if ui.button("Demander à l'assistant").clicked() { self.workshop_chat.open = true; }
                    } else {
                        sequence_cards(ui, steps, &[], &mut selection, &mut action, &mut insert_at, &validation_config);
                    }
                    ui.menu_button("+ Ajouter un bloc", |ui| {
                        for &(category, kinds) in BLOCK_CATEGORIES {
                            ui.menu_button(category, |ui| {
                                for &kind in kinds {
                                    if !self.preferences.workshop_advanced && kind == 18 { continue; }
                                    if ui.button(crate::automation::NAMES[kind]).clicked() { add = Some(kind); ui.close_menu(); }
                                }
                            });
                        }
                    });
                });
            }
            if let Err(e) = validate_key(&self.config.keys()[selected], &self.config) {
                ui.colored_label(Color32::LIGHT_RED, format!("Brouillon : {e}"));
            }
        });
        if let Some(kind) = add {
            let mut at = insert_at.take().unwrap_or_else(|| {
                let steps = &self.config.keys()[selected].steps;
                vec![selection.as_ref().and_then(|path| path.first()).map_or(steps.len(), |i| (i + 1).min(steps.len()))]
            });
            if let Some(new_path) = crate::workshop_ops::apply(&mut self.config.keys_mut()[selected].steps,
                crate::workshop_ops::Edit::Insert(std::mem::take(&mut at), new_block(kind, &profiles))) {
                selection = Some(new_path); changed = true;
            }
        }
        if let Some(operation) = action {
            match operation {
                WorkshopAction::Edit(edit) => {
                    if let Some(path) = crate::workshop_ops::apply(&mut self.config.keys_mut()[selected].steps, edit) {
                        selection = Some(path); changed = true;
                    }
                }
                WorkshopAction::Remove(path) => {
                    if crate::workshop_ops::remove(&mut self.config.keys_mut()[selected].steps, &path) {
                        selection = None; changed = true;
                    }
                }
            }
        }
        if ctx.input(|i| i.pointer.any_released()) {
            ctx.data_mut(|d| d.remove::<Vec<usize>>(egui::Id::new("workshop_drag")));
        }
        ctx.data_mut(|d| {
            if let Some(path) = insert_at { d.insert_temp(egui::Id::new("workshop_insert_at"), path); }
            else { d.remove::<Vec<usize>>(egui::Id::new("workshop_insert_at")); }
        });
        ctx.data_mut(|d| {
            if let Some(path) = selection {
                if path.len() == 1 { d.insert_temp(egui::Id::new("workshop_selected"), path[0]); }
                else { d.remove::<usize>(egui::Id::new("workshop_selected")); }
                d.insert_temp(path_id, path);
            } else {
                d.remove::<Vec<usize>>(path_id);
                d.remove::<usize>(egui::Id::new("workshop_selected"));
            }
        });
        if let Some((index, replace)) = template_action {
            let steps = &mut self.config.keys_mut()[self.selected].steps;
            let model = template(index);
            if replace { steps.clear(); ctx.data_mut(|d| d.remove::<Vec<usize>>(egui::Id::new("workshop_path"))); }
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
            ui.label("Actif sur la page actuellement ouverte. Les raccourcis restent disponibles sans périphérique branché.");
            let name=self.config.profiles[self.config.active].name.clone();let page=self.config.page.clone();let key=self.selected;
            let index=self.preferences.triggers.iter().position(|t|t.kind=="hotkey"&&t.profile==name&&t.page==page&&t.key==key);
            let mut text=index.map(|i|self.preferences.triggers[i].keys.clone()).unwrap_or_default();
            ui.horizontal(|ui|{if ui.text_edit_singleline(&mut text).changed(){hotkeys=true;}if ui.button("Enregistrer…").clicked(){capture=true;}});
            if !text.is_empty() { match crate::desktop::chord(&text) { Ok(_) => {}, Err(e) => { ui.colored_label(Color32::LIGHT_RED,e.to_string()); } } }
            ui.label(RichText::new("Exemple : CTRL+ALT+F9. Vide le champ pour désactiver. Windows signale les combinaisons déjà utilisées dans le journal.").small());
            if hotkeys {if let Some(i)=index {if text.is_empty(){self.preferences.triggers.remove(i);}else{self.preferences.triggers[i].keys=text;}}else if !text.is_empty()&&self.preferences.triggers.len()<256{self.preferences.triggers.push(crate::desktop::Trigger::hotkey(name.clone(),page.clone(),key,text));}changed=true;}
            ui.separator();
            ui.label(RichText::new("Déclencheurs automatiques").strong());
            ui.label("Surveillance toutes les secondes, sans action au démarrage. La pause globale bloque également ces déclencheurs.");
            egui::ComboBox::from_id_salt("automatic_trigger_kind").selected_text(match self.trigger_kind.as_str() {
                "app_open" => "Ouverture d’application", "app_close" => "Fermeture d’application",
                "clock" => "Heure fixe", "timer" => "Minuteur",
                "audio_connect" => "Branchement audio", _ => "Débranchement audio",
            }).show_ui(ui, |ui| {
                for (kind, label) in [("app_open", "Ouverture d’application"), ("app_close", "Fermeture d’application"), ("clock", "Heure fixe"), ("timer", "Minuteur"), ("audio_connect", "Branchement audio"), ("audio_disconnect", "Débranchement audio")] {
                    ui.selectable_value(&mut self.trigger_kind, kind.into(), label);
                }
            });
            match self.trigger_kind.as_str() {
                "timer" => { ui.horizontal(|ui| { ui.label("Toutes les (minutes)"); ui.add(egui::DragValue::new(&mut self.trigger_minutes).range(1..=1440)); }); },
                "clock" => {
                    ui.horizontal(|ui| { ui.label("Heure HH:MM"); ui.text_edit_singleline(&mut self.trigger_value); });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Jours (aucun = tous) :");
                        for (d, label) in ["Dim", "Lun", "Mar", "Mer", "Jeu", "Ven", "Sam"].iter().enumerate() {
                            let mut selected = self.trigger_days.contains(&(d as u8));
                            if ui.checkbox(&mut selected, *label).changed() {
                                if selected { self.trigger_days.push(d as u8); } else { self.trigger_days.retain(|&n| n != d as u8); }
                            }
                        }
                    });
                },
                _ => {
                    ui.horizontal(|ui| { ui.label(if self.trigger_kind.starts_with("app_") { "Nom .exe" } else { "Nom contient" }); ui.text_edit_singleline(&mut self.trigger_value); });
                    if self.trigger_kind.starts_with("audio_") { ui.checkbox(&mut self.trigger_input, "Périphérique d’entrée (microphone)"); }
                }
            }
            if ui.add(crate::theme::secondary_button(format!("Ajouter → touche {:02} (profil et page actuels)", key + 1))).clicked() {
                let trigger = crate::desktop::Trigger { profile: name.clone(), page: page.clone(), key, keys: String::new(), kind: self.trigger_kind.clone(), value: self.trigger_value.clone(), days: self.trigger_days.clone(), minutes: self.trigger_minutes, input: self.trigger_input };
                if let Err(e) = trigger.validate() { self.fail(format!("Déclencheur : {e}")); }
                else if self.preferences.triggers.len() >= 256 { self.fail("256 déclencheurs maximum".into()); }
                else { self.preferences.triggers.push(trigger); changed = true; }
            }
            let mut remove = None;
            for (i, trigger) in self.preferences.triggers.iter().enumerate().filter(|(_, t)| t.kind != "hotkey") {
                ui.horizontal(|ui| {
                    ui.label(format!("{} → {} / {} / touche {:02}", trigger.description(), trigger.profile, if trigger.page.is_empty() { "Accueil" } else { &trigger.page }, trigger.key + 1));
                    if ui.add(crate::theme::danger_button("Supprimer")).clicked() { remove = Some(i); }
                });
            }
            if let Some(i) = remove { self.preferences.triggers.remove(i); changed = true; }
            if self.trigger_recording{ui.colored_label(accent(),"Appuie sur la combinaison souhaitée…");if ui.button("Annuler").clicked(){self.trigger_recording=false;self.refresh_hotkeys();}}
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
                        .find(|t| t.kind == "hotkey" && t.profile == name && t.page == page && t.key == key)
                    {
                        t.keys = keys;
                    } else if self.preferences.triggers.len() < 256 {
                        self.preferences.triggers.push(crate::desktop::Trigger::hotkey(name, page, key, keys));
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
        let steps = vec![Step::SetVariable { name: "mode".into(), value: "travail".into(), persistent: false, },
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
        assert_eq!(block_summary(&crate::automation::step(43)), "Mettre texte en majuscules");
        assert_eq!(block_summary(&Step::Repeat { count: 3, steps: vec![Step::Back, Step::Back] }), "Répéter 3 fois (2 blocs)");
    }
}
