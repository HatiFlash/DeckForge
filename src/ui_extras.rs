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
fn plugin_step(plugin: &crate::plugins::Plugin, action: &crate::plugins::Action) -> Step {
    Step::PluginAction { plugin:plugin.uuid.clone(), action:action.uuid.clone(),
        settings:serde_json::json!({}), label:action.name.clone() }
}
fn plugin_choices(ui:&mut egui::Ui,plugins:&[crate::plugins::Plugin],search:&str) -> Option<Step> {
    let mut chosen=None;
    egui::CollapsingHeader::new(crate::i18n::tr("Plugins")).default_open(!search.is_empty()).show(ui,|ui| {
        for plugin in plugins.iter().filter(|p| p.prerequisite.is_none() && matches!(p.runtime,crate::plugins::Runtime::Exe(_)|crate::plugins::Runtime::Node(_)|crate::plugins::Runtime::Html(_))) {
            for action in &plugin.actions {
                let title=format!("{} › {}",plugin.display_name(),if action.name.is_empty(){&action.uuid}else{&action.name});
                if !search.is_empty() && !title.to_lowercase().contains(&search.to_lowercase()){continue;}
                ui.horizontal(|ui| {
                    if let Some(texture)=action.icon.as_ref().or(plugin.icon.as_ref()).and_then(|p|super::icon_ui::texture(ui.ctx(),&p.to_string_lossy())) {
                        ui.image((texture.id(),egui::vec2(16.0,16.0)));
                    }
                    if ui.button(&title).clicked(){chosen=Some(plugin_step(plugin,action));}
                });
            }
        }
    });
    chosen
}
fn plugin_fields(ui:&mut egui::Ui,step:&mut Step,config:&crate::model::Config,plugins:&[crate::plugins::Plugin],advanced:bool)->bool {
    let Step::PluginAction {plugin,action,settings,label}=step else{return false};
    let mut changed=false;
    if let Some(p)=plugins.iter().find(|p|p.uuid==*plugin) {
        if let Some(a)=p.actions.iter().find(|a|a.uuid==*action) {
            ui.label(format!("{} › {}",p.display_name(),if a.name.is_empty(){&a.uuid}else{&a.name}));
        } else {ui.colored_label(theme::palette(ui.ctx()).warning,crate::i18n::tr("Action de plugin introuvable"));}
    } else {ui.colored_label(theme::palette(ui.ctx()).warning,crate::i18n::tr("Plugin introuvable"));}
    ui.menu_button(crate::i18n::tr("Copier les réglages d'une touche…"), |ui| {
        for (index,key) in config.keys().iter().enumerate() {
            if let Some(binding)=key.plugin.as_ref().filter(|b|b.plugin==*plugin && b.action==*action) {
                let title=crate::i18n::trf("Touche {index} : {label}",&[("index",&(index+1)),("label",&key.label)]);
                if ui.button(title).clicked(){*settings=binding.settings.clone();changed=true;ui.close_menu();}
            }
        }
    });
    if advanced {
        ui.label(crate::i18n::tr("Réglages JSON (objet)"));
        let id=ui.id().with(("plugin-json",plugin.as_str(),action.as_str()));
        let mut text=ui.data(|d|d.get_temp::<String>(id)).unwrap_or_else(||serde_json::to_string_pretty(settings).unwrap_or_else(|_|"{}".into()));
        if ui.add(egui::TextEdit::multiline(&mut text).desired_rows(4)).changed(){
            if let Ok(value)=serde_json::from_str::<serde_json::Value>(&text) {if value.is_object(){*settings=value;changed=true;}}
            ui.data_mut(|d|d.insert_temp(id,text.clone()));
        }
        if !serde_json::from_str::<serde_json::Value>(&text).is_ok_and(|v|v.is_object()) {
            ui.colored_label(theme::palette(ui.ctx()).warning,crate::i18n::tr("Un objet JSON valide est requis."));
        }
    }
    let _=label;
    changed
}
const BLOCK_CATEGORIES: &[(&str, &[usize])] = &[
    ("Clavier", &[0, 45]),
    ("Streaming", &[19, 20, 21, 22, 23, 24, 25, 30, 31, 0, 4]), ("Système", &[0, 2, 4, 26, 27, 28, 29, 32, 33]),
    ("Texte", &[1, 43, 44]), ("Applications et sites", &[2, 3]),
    ("Médias et audio", &[4, 13, 37, 38]), ("Fenêtres", &[6, 46]), ("Attendre", &[5, 40]), ("Notifications", &[41]),
    ("Logique et répétitions", &[14, 15, 39]), ("Variables", &[16, 17, 42]),
    ("Navigation NovyDeck", &[7, 8, 9, 10, 11]), ("Avancé / AutoHotkey", &[18]),
];
fn block_name(kind: usize) -> &'static str {
    match kind {
        0 => crate::i18n::tr("Raccourci clavier"),
        1 => crate::i18n::tr("Saisir du texte"),
        2 => crate::i18n::tr("Application"),
        3 => crate::i18n::tr("Site web"),
        4 => crate::i18n::tr("Média / volume"),
        5 => crate::i18n::tr("Attendre"),
        6 => crate::i18n::tr("Attendre une fenêtre"),
        7 => crate::i18n::tr("Changer de profil"),
        8 => crate::i18n::tr("Page suivante"),
        9 => crate::i18n::tr("Page précédente"),
        10 => crate::i18n::tr("Retour au dossier"),
        11 => crate::i18n::tr("Page 1"),
        12 => crate::i18n::tr("Créer un dossier"),
        13 => crate::i18n::tr("Basculer l’audio"),
        14 => crate::i18n::tr("Si une fenêtre est ouverte"),
        15 => crate::i18n::tr("Répéter des blocs"),
        16 => crate::i18n::tr("Définir une variable"),
        17 => crate::i18n::tr("Si une variable correspond"),
        18 => crate::i18n::tr("Script AutoHotkey"),
        19 => crate::i18n::tr("OBS : scène"),
        20 => crate::i18n::tr("OBS : enregistrement"),
        21 => crate::i18n::tr("OBS : diffusion"),
        22 => crate::i18n::tr("OBS : caméra"),
        23 => crate::i18n::tr("OBS : replay"),
        24 => crate::i18n::tr("OBS : micro"),
        25 => crate::i18n::tr("OBS : source"),
        26 => crate::i18n::tr("Fermer la fenêtre active"),
        27 => crate::i18n::tr("Clic souris"),
        28 => crate::i18n::tr("Déplacer la souris"),
        29 => crate::i18n::tr("Défilement souris"),
        30 => crate::i18n::tr("Jouer un son"),
        31 => crate::i18n::tr("Arrêter les sons"),
        32 => crate::i18n::tr("Luminosité du périphérique"),
        33 => crate::i18n::tr("Veille du périphérique"),
        34 => crate::i18n::tr("Envoyer en UDP"),
        35 => crate::i18n::tr("Requête HTTP"),
        36 => crate::i18n::tr("Touche à deux états"),
        37 => crate::i18n::tr("Volume d’une application"),
        38 => crate::i18n::tr("Volume d’un périphérique"),
        39 => crate::i18n::tr("Si… sinon"),
        40 => crate::i18n::tr("Attendre que…"),
        41 => crate::i18n::tr("Notification"),
        42 => crate::i18n::tr("Calculer"),
        43 => crate::i18n::tr("Texte"),
        44 => crate::i18n::tr("Presse-papiers"),
        45 => crate::i18n::tr("Maintenir une touche"),
        46 => crate::i18n::tr("Opération sur une fenêtre"),
        _ => crate::automation::NAMES[kind],
    }
}
pub(super) fn step_label(step: &Step) -> &'static str {
    let name = step.name();
    match name {
        "Action de plugin" => crate::i18n::tr("Action de plugin"),
        "Raccourci" => crate::i18n::tr("Raccourci"),
        "Lancer une application" => crate::i18n::tr("Lancer une application"),
        "Ouvrir un site" => crate::i18n::tr("Ouvrir un site"),
        "Aller à une page" => crate::i18n::tr("Aller à une page"),
        "Ouvrir un dossier" => crate::i18n::tr("Ouvrir un dossier"),
        "Basculer le périphérique audio" => crate::i18n::tr("Basculer le périphérique audio"),
        "Basculer entre deux actions" => crate::i18n::tr("Basculer entre deux actions"),
        "Mettre le périphérique en veille" => crate::i18n::tr("Mettre le périphérique en veille"),
        "OBS : caméra virtuelle" => crate::i18n::tr("OBS : caméra virtuelle"),
        "OBS : couper une entrée" => crate::i18n::tr("OBS : couper une entrée"),
        "OBS : visibilité de source" => crate::i18n::tr("OBS : visibilité de source"),
        "Retour au dossier précédent" => crate::i18n::tr("Retour au dossier précédent"),
        _ => crate::automation::NAMES.iter().position(|n| *n == name).map_or(name, block_name),
    }
}
fn category_name(category: &str) -> &'static str {
    match category {
        "Clavier" => crate::i18n::tr("Clavier"),
        "Streaming" => crate::i18n::tr("Streaming"),
        "Système" => crate::i18n::tr("Système"),
        "Texte" => crate::i18n::tr("Texte"),
        "Applications et sites" => crate::i18n::tr("Applications et sites"),
        "Médias et audio" => crate::i18n::tr("Médias et audio"),
        "Fenêtres" => crate::i18n::tr("Fenêtres"),
        "Attendre" => crate::i18n::tr("Attendre"),
        "Notifications" => crate::i18n::tr("Notifications"),
        "Logique et répétitions" => crate::i18n::tr("Logique et répétitions"),
        "Variables" => crate::i18n::tr("Variables"),
        "Navigation NovyDeck" => crate::i18n::tr("Navigation NovyDeck"),
        "Avancé / AutoHotkey" => crate::i18n::tr("Avancé / AutoHotkey"),
        _ => "",
    }
}
fn template_name(index: usize) -> &'static str {
    match index {
        0 => crate::i18n::tr("Ouvrir une application puis un site"),
        1 => crate::i18n::tr("Copier puis coller ailleurs"),
        2 => crate::i18n::tr("Taper une signature"),
        3 => crate::i18n::tr("Basculer casque / enceintes"),
        4 => crate::i18n::tr("Mode travail"),
        5 => crate::i18n::tr("Répéter une touche N fois"),
        _ => "",
    }
}
fn block_help(kind: usize) -> &'static str {
    match kind {
        27 => crate::i18n::tr("Clique à la position actuelle, ou à des coordonnées écran, zone client ou curseur."),
        28 => crate::i18n::tr("Déplace la souris dans l’écran, depuis le curseur ou dans la zone client capturée au déclenchement."),
        29 => crate::i18n::tr("Défile verticalement ou horizontalement ; simulation sans effet."),
        46 => crate::i18n::tr("Active, déplace ou redimensionne une fenêtre ; jamais NovyDeck sauf ciblage explicite par titre."),
        45 => crate::i18n::tr("Maintient des touches ; 0 ms jusqu’au bloc Relâcher, fin ou annulation de la macro."),
        0 => crate::i18n::tr("Appuie sur une combinaison de touches."), 1 => crate::i18n::tr("Écrit du texte dans la fenêtre active ; accepte {nom} et les variables spéciales."),
        2 => crate::i18n::tr("Démarre un programme Windows (arguments interpolés)."), 3 => crate::i18n::tr("Ouvre une adresse web (gabarit interpolé)."),
        4 => crate::i18n::tr("Commande la lecture et le volume."), 5 => crate::i18n::tr("Attend avant le bloc suivant."),
        6 => crate::i18n::tr("Attend et peut activer une fenêtre."), 13 => crate::i18n::tr("Bascule entre deux appareils audio."), 37 => crate::i18n::tr("Règle uniquement les sessions audio de l’application ciblée."), 38 => crate::i18n::tr("Règle le volume du périphérique (sortie ou microphone), et non celui d’une application."),
        39 => crate::i18n::tr("Évalue une condition en lecture seule ; exécute ALORS ou SINON."),
        40 => crate::i18n::tr("Attend une condition Windows en lecture seule, avec délai et bouton Arrêter."),
        41 => crate::i18n::tr("Affiche un message Windows sans voler le focus ; accepte les variables de macro.") ,
        14 => crate::i18n::tr("Exécute des blocs si une fenêtre est ouverte."), 15 => crate::i18n::tr("Rejoue plusieurs fois les blocs imbriqués."),
        42 => crate::i18n::tr("Calcule une valeur numérique dans une variable de la macro."),
        43 => crate::i18n::tr("Transforme une variable texte pendant cette macro, sans saisir au clavier."),
        44 => crate::i18n::tr("Copie, lit, vide, sauvegarde ou restaure le texte Unicode du presse-papiers."),
        16 => crate::i18n::tr("Enregistre une valeur pour cette exécution."), 17 => crate::i18n::tr("Choisit des blocs selon une valeur."),
        19..=25 => crate::i18n::tr("Commande OBS Studio par WebSocket local (OBS 28 ou plus)."),
        32 => crate::i18n::tr("Règle ou ajuste la luminosité du périphérique (0 à 100 %)."),
        33 => crate::i18n::tr("Met le périphérique en veille ; le prochain appui le réveille."),
        18 => crate::i18n::tr("Exécute un script AutoHotkey v2."), _ => crate::i18n::tr("Change la page, le dossier ou le profil affiché."),
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
    search.is_empty() || format!("{} {} {}", category, block_name(kind), aliases).to_lowercase().contains(&search.to_lowercase())
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
        egui::CollapsingHeader::new(category_name(category)).default_open(search.is_empty() && category == "Clavier" || !search.is_empty())
            .show(ui, |ui| {
                for kind in matches {
                    ui.horizontal(|ui| {
                        let ico = iconset::for_step(&crate::automation::step(kind));
                        family_bar(ui, iconset::family_color(ui.ctx(), ico));
                        if let Some(image) = iconset::image(ui.ctx(), ico, 16.0) {
                            ui.add(image);
                        }
                        let _ = category;
                        let label = if keyboard == Some(index) { format!("> {}", block_name(kind)) }
                            else { block_name(kind).to_owned() };
                        if ui.add(egui::Button::new(label).frame(false))
                            .on_hover_text(block_help(kind)).clicked() { selected = Some(kind); }
                        index += 1;
                    });
                }
            });
    }
    if !found { ui.label(crate::i18n::tr("Aucun bloc trouvé. Essaie un autre mot-clé.")); }
    selected
}
fn simple_fields_visible(step: &Step, advanced: bool) -> bool {
    advanced || !matches!(step.active(), Step::AutoHotkey { .. })
}
/// Résumé avec les noms lisibles des plugins installés (« NovyDeck Clock › Stopwatch »).
fn named_summary(step: &Step, plugins: &[crate::plugins::Plugin]) -> String {
    if let Step::PluginAction { plugin, action, label, .. } = step.active() {
        let owner = plugins.iter().find(|p| &p.uuid == plugin);
        let plugin_name = owner.map(|p| p.display_name()).unwrap_or(plugin);
        let action_name = owner.and_then(|p| p.actions.iter().find(|a| &a.uuid == action))
            .map(|a| if a.name.is_empty() { action.as_str() } else { a.name.as_str() })
            .unwrap_or_else(|| if label.is_empty() { action.as_str() } else { label.as_str() });
        format!("{plugin_name} › {action_name}")
    } else { block_summary(step) }
}
pub(super) fn block_summary(step: &Step) -> String {
    match step.active() {
        Step::PluginAction { plugin, action, label, .. } => crate::i18n::trf("Plugin {plugin} › {action}", &[("plugin",plugin),("action",&if label.is_empty(){action.clone()}else{label.clone()})]),
        Step::MouseClick { .. } | Step::MouseMove { .. } | Step::MouseScroll { .. } => super::step_summary(step),
        Step::Clipboard { .. } => step.clipboard_summary().unwrap_or_default(),
        Step::WindowOp { .. } => step.window_op_summary().unwrap_or_default(),
        Step::AppVolume { .. } => step.app_volume_summary().unwrap_or_default(),
        Step::DeviceVolume { .. } => step.device_volume_summary().unwrap_or_default(),
        Step::ObsScene { name } => crate::i18n::trf("OBS : scène {name}", &[("name", &(name))] ),
        Step::ObsRecord { mode } | Step::ObsStream { mode } | Step::ObsVirtualCam { mode } | Step::ObsReplay { mode } => format!("{} : {mode}", step.name()),
        Step::ObsMute { input, mode } => crate::i18n::trf("OBS : {input} ({mode})", &[("input", &(input)), ("mode", &(mode))] ),
        Step::ObsSourceVisibility { scene, source, mode } => crate::i18n::trf("OBS : {scene} / {source} ({mode})", &[("scene", &(scene)), ("source", &(source)), ("mode", &(mode))] ),
        Step::Hotkey { keys } => crate::i18n::trf("Appuyer sur {keys}", &[("keys", &(keys))] ),
        Step::KeyHold { .. } => step.key_hold_summary().unwrap_or_default(),
        Step::Delay { ms } => crate::i18n::trf("Attendre {ms} ms", &[("ms", &(ms))] ),
        Step::Url { url } => crate::i18n::trf("Ouvrir {url}", &[("url", &(url))] ),
        Step::Text { text } => crate::i18n::trf("Écrire {text}", &[("text", &(text.chars().take(50).collect::<String>()))] ),
        Step::Launch { program, .. } => crate::i18n::trf("Lancer {program}", &[("program", &(program))] ),
        Step::Toggle { a, b } => crate::i18n::trf("Basculer : {a} / {b} blocs", &[("a", &(a.len())), ("b", &(b.len()))] ),
        Step::Repeat { count, steps } => crate::i18n::trf("Répéter {count} fois ({blocks} blocs)", &[("count", &(count)), ("blocks", &(steps.len()))] ),
        Step::UdpSend { host, port, .. } => crate::i18n::trf("UDP : {host}:{port}", &[("host", &(host)), ("port", &(port))] ),
        Step::HttpRequest { method, url, .. } => crate::i18n::trf("HTTP {method} : {url}", &[("method", &(method)), ("url", &(url))] ),
        Step::D6Brightness { mode, value } => crate::i18n::trf("Luminosité : {mode} {value} %", &[("mode", &(mode)), ("value", &(value))] ),
        Step::IfCondition { condition, .. } => crate::i18n::trf("Si {condition}", &[("condition", &(condition.summary()))] ),
        Step::WaitUntil { condition, timeout_ms, .. } => crate::i18n::trf("Attendre que {condition} ({timeout} ms max)", &[("condition", &(condition.summary())), ("timeout", &(timeout_ms))] ),
        Step::Notify { title, message } => crate::i18n::trf("Notification « {title} » : {message}", &[("title", &(title)), ("message", &(message.chars().take(50).collect::<String>()))] ),
        Step::IfWindow { title, .. } => crate::i18n::trf("Si la fenêtre {title} est ouverte", &[("title", &(title))] ),
        Step::IfVariable { name, value, .. } => crate::i18n::trf("Si {name} vaut {value}", &[("name", &(name)), ("value", &(value))] ),
        Step::Math { .. } => step.math_summary().unwrap_or_default(),
        Step::TextOp { .. } => step.text_op_summary().unwrap_or_default(),
        Step::SetVariable { name, value, persistent } => crate::i18n::trf("Mémoriser {name} = {value}{persistent}", &[("name", &(name)), ("value", &(value)), ("persistent", &(if *persistent { crate::i18n::tr(" (enregistrée)") } else { "" }))] ),
        Step::WaitWindow { title, .. } => crate::i18n::trf("Attendre la fenêtre {title}", &[("title", &(title))] ),
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
    iconset::menu(ui, Ico::Template, crate::i18n::tr("Modèles"), |ui| {
        for (i, name) in TEMPLATE_NAMES.iter().enumerate() {
            if ui.button(template_name(i)).clicked() { *choice = Some(i); ui.close_menu(); }
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
                out.push(TraceLine { path, text: crate::i18n::trf("{prefix} · {block} — désactivé, sauté", &[("prefix", &(prefix)), ("block", &(block_summary(step)))] ) });
                continue;
            }
            let detail = match step {
                Step::Delay { ms } => crate::i18n::trf("{ms} ms simulées", &[("ms", &(ms))] ),
                Step::WaitWindow { timeout_ms, .. } => crate::i18n::trf("jusqu'à {timeout_ms} ms (sans attendre)", &[("timeout_ms", &(timeout_ms))] ),
                Step::WaitUntil { .. } => crate::i18n::tr("condition lue une fois ; sans attendre").into(),
                Step::IfCondition { .. } => crate::i18n::tr("ALORS et SINON explorés ; condition lue sans effet").into(),
                Step::IfWindow { .. } => crate::i18n::tr("ALORS et SINON explorés (fenêtre non évaluée)").into(),
                Step::IfVariable { .. } => crate::i18n::tr("ALORS et SINON explorés (valeur non évaluée)").into(),
                Step::Repeat { count, .. } => crate::i18n::trf("1 itération simulée sur {count} prévues", &[("count", &(count))] ),
                Step::Toggle { .. } => crate::i18n::tr("États 1 et 2 explorés sans effet").into(),
                _ => crate::i18n::tr("0 ms (action non exécutée)").into(),
            };
            out.push(TraceLine { path: path.clone(), text: crate::i18n::trf("{prefix} · {block} — {detail}", &[("prefix", &(prefix)), ("block", &(block_summary(step))), ("detail", &(detail))] ) });
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
    plugins: &[crate::plugins::Plugin],
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
                ui.menu_button(crate::i18n::tr("+ Insérer ici"), |ui| {
                    for &(category, kinds) in BLOCK_CATEGORIES {
                        ui.menu_button(category_name(category), |ui| {
                            for &kind in kinds {
                                if depth > 0 && (7..=11).contains(&kind) || !advanced && kind == 18 { continue; }
                                if ui.button(block_name(kind)).on_hover_text(block_help(kind)).clicked() {
                                    insertion = Some((i, new_block(kind, profiles)));
                                    ui.close_menu();
                                }
                            }
                        });
                    }
                    if let Some(plugin_step) = plugin_choices(ui, plugins, "") {
                        insertion = Some((i, plugin_step));
                        ui.close_menu();
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
                        let summary = named_summary(step, plugins);
                        if ui.selectable_label(!folded, if summary.starts_with(step_label(step)) { summary.clone() } else { format!("{} · {}", step_label(step), summary) })
                            .on_hover_text(format!("{} — {}", step_label(step), summary)).clicked() {
                            ui.data_mut(|d| d.insert_temp(folded_id, !folded));
                            if parent.is_empty() {
                                ui.data_mut(|d| d.insert_temp(egui::Id::new("workshop_selected"), i));
                            }
                        }
                        ui.menu_button("...", |ui| {
                            if ui.button(crate::i18n::tr("Dupliquer")).clicked() { duplicate = Some(i); ui.close_menu(); }
                            if ui.button(crate::i18n::tr("Retirer")).clicked() { remove = Some(i); ui.close_menu(); }
                            if ui.button(if disabled { crate::i18n::tr("Activer") } else { crate::i18n::tr("Désactiver") }).clicked() { toggle = Some(i); ui.close_menu(); }
                            if ui.add_enabled(i > 0, egui::Button::new(crate::i18n::tr("Monter"))).clicked() { movement = Some((i, i - 1)); ui.close_menu(); }
                            if ui.add_enabled(i + 1 < step_count, egui::Button::new(crate::i18n::tr("Descendre"))).clicked() { movement = Some((i, i + 1)); ui.close_menu(); }
                        });
                    });
                    if let Some(e) = &error { if !disabled { ui.colored_label(Color32::from_rgb(255, 170, 70), crate::i18n::trf("À compléter : {e}", &[("e", &(e))] )); } }
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
                    if matches!(step.active(), Step::PluginAction { .. }) {
                        changed |= plugin_fields(ui, step.active_mut(), config, plugins, advanced);
                    } else if simple_fields_visible(step.active(), advanced) {
                        match step.active_mut() {
                            Step::Launch { program, .. } if !advanced => {
                                ui.label(crate::i18n::tr("Programme"));
                                changed |= ui.text_edit_singleline(program).changed();
                            }
                            Step::WaitWindow { title, .. } if !advanced => {
                                ui.label(crate::i18n::tr("Titre de la fenêtre"));
                                changed |= ui.text_edit_singleline(title).changed();
                            }
                            _ => {
                                changed |= edit_step(ui, step.active_mut(), profiles, pages, &mut capture, audio, &mut icon, tx, &(target.0,target.1.clone(),target.2,path.clone()));
                            }
                        }
                    } else { ui.label(crate::i18n::tr("Paramètres AutoHotkey disponibles en mode Avancé.")); }
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
                                ui.label(RichText::new(if branch == 0 { crate::i18n::tr("ALORS") } else { crate::i18n::tr("SINON") }).small().strong().color(color));
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
                                        plugins,
                                    );
                                });
                            }
                        }
                        Step::Toggle { a, b } => {
                            for (branch, label, list) in [(0, "ÉTAT 1", a), (1, "ÉTAT 2", b)] {
                                ui.label(if branch == 0 { crate::i18n::tr("ÉTAT 1") } else { crate::i18n::tr("ÉTAT 2") });
                                let mut child = path.clone(); child.push(branch);
                                ui.indent(branch, |ui| { changed |= block_editor(ui, list, profiles, pages, audio, &child, record, depth + 1, running, pick_audio, tx, target, config, advanced, plugins); });
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
                                    plugins,
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
    if let Some((i, new_step)) = insertion {
        if steps.len() < 256 {
            steps.insert(i, new_step);
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
        ui.menu_button(crate::i18n::tr("+ Ajouter un bloc"), |ui| {
            for (i, label) in crate::automation::NAMES.iter().enumerate() {
                if i == 12 || depth > 0 && (7..=11).contains(&i) || !advanced && i == 18 {
                    continue;
                }
                if ui.button(block_name(i)).clicked() {
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
// Icône IconPark du bloc ; repli sur les petits tracés vectoriels si elle ne peut pas être rendue.
fn block_glyph(ui: &mut egui::Ui, step: &Step, color: Color32) {
    if let Some(image) = iconset::image(ui.ctx(), iconset::for_step(step), 18.0) {
        ui.add(image);
        return;
    }
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
    plugins: &[crate::plugins::Plugin],
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
                        let summary = named_summary(step, plugins);
                        // Le glyphe identifie déjà un bloc de plugin : « Plugin › action » suffit.
                        let title = if summary.starts_with(step_label(step)) || matches!(step.active(), Step::PluginAction { .. }) { summary.clone() } else { format!("{} · {}", step_label(step), summary) };
                        let label = ui.add(egui::Button::new(egui::RichText::new(title)).selected(chosen)
                            .truncate().sense(egui::Sense::click_and_drag()))
                            .on_hover_text(crate::i18n::trf("{name} : {summary}. Cliquer pour afficher les propriétés ; glisser pour déplacer.", &[("name", &step_label(step)), ("summary", &summary)]));
                        if label.clicked() { *selection = if chosen { None } else { Some(path.clone()) }; }
                        if label.drag_started() { ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("workshop_drag"), path.clone())); }
                        if let Some(e) = &error { ui.colored_label(Color32::LIGHT_RED, crate::i18n::tr("Erreur")).on_hover_text(e.to_string()); }
                        ui.menu_button("...", |ui| {
                            if ui.button(crate::i18n::tr("Dupliquer")).clicked() { *action = Some(WorkshopAction::Edit(crate::workshop_ops::Edit::Duplicate(path.clone()))); ui.close_menu(); }
                            if ui.button(crate::i18n::tr("Supprimer")).clicked() { *action = Some(WorkshopAction::Remove(path.clone())); ui.close_menu(); }
                            if ui.add_enabled(i > 0, egui::Button::new(crate::i18n::tr("Monter"))).clicked() {
                                *action = Some(WorkshopAction::Edit(crate::workshop_ops::Edit::Move(path.clone(), i - 1))); ui.close_menu();
                            }
                            if ui.add_enabled(i + 1 < steps.len(), egui::Button::new(crate::i18n::tr("Descendre"))).clicked() {
                                *action = Some(WorkshopAction::Edit(crate::workshop_ops::Edit::Move(path.clone(), i + 1))); ui.close_menu();
                            }
                            if ui.button(if step.is_disabled() { crate::i18n::tr("Réactiver") } else { crate::i18n::tr("Désactiver") }).clicked() {
                                *action = Some(WorkshopAction::Edit(crate::workshop_ops::Edit::Toggle(path.clone()))); ui.close_menu();
                            }
                            if ui.button(crate::i18n::tr("Insérer un bloc après")).clicked() {
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
                            ui.label(RichText::new(match label { "RÉPÉTER" => crate::i18n::tr("RÉPÉTER"), "ÉTAT 1" => crate::i18n::tr("ÉTAT 1"), "ÉTAT 2" => crate::i18n::tr("ÉTAT 2"), "ALORS" => crate::i18n::tr("ALORS"), "SINON" => crate::i18n::tr("SINON"), _ => label }).small().color(p.muted));
                            let mut nested = path.clone(); nested.push(branch);
                            sequence_cards(ui, children, &nested, selection, action, insert_at, config, plugins);
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
        format!("Utilise le connecteur MCP NovyDeck pour proposer cette automatisation :\n{}\n\nDestination exacte : {destination}\nLis d’abord deckforge_get_state et deckforge_get_profile. Si la demande concerne l’audio, utilise deckforge_get_audio_devices pour récupérer les identifiants réels. Tu peux utiliser deckforge_get_icons ; pour audio_switch, first_icon/second_icon acceptent builtin:headphones, builtin:speakers et builtin:microphone. Crée les étapes avec deckforge_configure_key (conditions, répétitions et variables possibles). Si une information manque, demande-la-moi au lieu d’inventer un périphérique, un programme ou un script. N’exécute aucune macro. Attends la validation de la proposition dans NovyDeck ; les blocs resteront ensuite modifiables dans l’Atelier.",self.assistant_request.trim())
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
        egui::Window::new(crate::i18n::tr("Variables enregistrées")).open(&mut open).resizable(true).show(ctx, |ui| {
            ui.label(crate::i18n::tr("Conservées dans variables.json ; {nom} les utilise dans les macros suivantes."));
            let mut remove = None;
            for (name, original) in crate::persistent_variables::snapshot() {
                ui.horizontal(|ui| {
                    ui.label(format!("{name} :"));
                    let mut value = original.clone();
                    if ui.text_edit_singleline(&mut value).changed() && value != original {
                        if let Err(e) = crate::persistent_variables::set(&name, &value) {
                            self.fail(crate::i18n::trf("Variable non enregistrée : {e}", &[("e", &(e))] ));
                        }
                    }
                    if ui.add(theme::danger_button(crate::i18n::tr("Supprimer"))).clicked() { remove = Some(name); }
                });
            }
            if let Some(name) = remove {
                if let Err(e) = crate::persistent_variables::remove(&name) {
                    self.fail(crate::i18n::trf("Variable non supprimée : {e}", &[("e", &(e))] ));
                }
            }
        });
        self.show_saved_variables = open;
    }
    pub(super) fn start_workshop_run(&mut self, dry: bool) {
        if self.engine.running.load(Ordering::SeqCst) { return; }
        let mut key = self.config.keys()[self.selected].clone();
        if key.steps.is_empty() { self.fail(crate::i18n::tr("Ajoute un bloc avant de tester.").into()); return; }
        if let Err(e) = validate_key(&key, &self.config) { self.fail(crate::i18n::trf("Action à corriger : {e}", &[("e", &(e))] )); return; }
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
        let selection_before = selection.clone();
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
                if ui.button(crate::i18n::tr("← Retour au deck")).clicked() { self.show_workflow = false; selection = None; }
                let key = &self.config.keys()[self.selected];
                ui.label(RichText::new(format!("Atelier · Touche {:02}{}", self.selected + 1,
                    if key.label.is_empty() { String::new() } else { format!(" · {}", key.label) })).strong());
                ui.separator();
                if ui.selectable_value(&mut self.preferences.workshop_advanced, false, crate::i18n::tr("Simple")).changed() { self.save_preferences(); }
                if ui.selectable_value(&mut self.preferences.workshop_advanced, true, crate::i18n::tr("Avancé")).changed() { self.save_preferences(); }
                if compact && ui.button(if palette_open { crate::i18n::tr("Masquer les blocs") } else { crate::i18n::tr("Blocs") }).clicked() { palette_open = !palette_open; }
                template_menu(ui, &mut template_choice);
                if self.preferences.workshop_advanced && ui.button(crate::i18n::tr("Exporter AHK")).clicked() { export = true; }
                if ui.add(egui::Button::new(crate::i18n::tr("Enregistrer")).fill(accent())).clicked() { self.save(); }
            });
            if let Some(index) = template_choice {
                let existing = self.config.keys()[self.selected].steps.len();
                if existing == 0 { template_action = Some((index, false)); }
                else {
                    ui.label(crate::i18n::tr("Cette touche contient déjà des blocs. Choisis comment utiliser le modèle :"));
                    ui.horizontal(|ui| {
                        if ui.button(crate::i18n::tr("Ajouter à la fin")).clicked() { template_action = Some((index, false)); }
                        if ui.button(crate::i18n::tr("Remplacer les blocs")).clicked() { template_action = Some((index, true)); }
                    });
                }
            }
        });
        let search_id = egui::Id::new("workshop_search");
        let mut search = ctx.data(|d| d.get_temp::<String>(search_id)).unwrap_or_default();
        let mut add = None;
        let mut add_plugin = None;
        let installed_plugins = self.plugins.as_ref().map(|m|m.catalog.clone()).unwrap_or_default();
        ctx.data_mut(|d| d.insert_temp(palette_id, palette_open));
        if (!compact || palette_open) && !(compact && self.workshop_chat.open) {
        egui::SidePanel::left("workshop_library").resizable(!compact).default_width(if compact { 240.0 } else { 280.0 }).min_width(200.0).max_width(if compact { 260.0 } else { 350.0 }).show(ctx, |ui| {
            ui.label(RichText::new(crate::i18n::tr("BIBLIOTHÈQUE DE BLOCS")).strong());
            let response = ui.add(egui::TextEdit::singleline(&mut search).hint_text(crate::i18n::tr("Rechercher un bloc")));
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
                add_plugin = plugin_choices(ui, &installed_plugins, &search);
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
                let arrow = if self.workshop_journal.open { crate::i18n::tr("Masquer") } else { crate::i18n::tr("Journal") };
                if ui.add(iconset::button(ui.ctx(), Ico::Log, arrow)).clicked() { self.workshop_journal.open = !self.workshop_journal.open; }
                ui.add(egui::Label::new(self.workshop_journal.summary()).truncate());
                if ui.add_enabled(can_run, iconset::button(ui.ctx(), Ico::Play, crate::i18n::tr("Simuler"))).on_hover_text(crate::i18n::tr("Déroule les actions dans le journal, sans aucun effet sur le PC")).clicked() { self.start_workshop_run(true); }
                if ui.add_enabled(can_run, iconset::button(ui.ctx(), Ico::Actions, crate::i18n::tr("Tester pour de vrai"))).on_hover_text(crate::i18n::tr("Exécute réellement les actions, après confirmation")).clicked() {
                    self.workshop_journal.confirm_real = true;
                }
                if ui.add(iconset::button(ui.ctx(), Ico::Robot, if self.workshop_chat.open { crate::i18n::tr("Fermer l’assistant") } else { crate::i18n::tr("Assistant") })).clicked() { self.workshop_chat.open = !self.workshop_chat.open; }
            });
            if self.workshop_journal.open {
                ui.separator();
                ui.label(if self.workshop_journal.mode == Some(false) { crate::i18n::tr("EXÉCUTION RÉELLE") } else { crate::i18n::tr("SIMULATION") });
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
                        if ui.small_button(crate::i18n::tr("× Fermer les propriétés")).clicked() { close = true; }
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            if let Some(step) = step_mut(&mut self.config.keys_mut()[selected].steps, &path) {
                                ui.label(RichText::new(crate::i18n::tr("Propriétés du bloc")).strong());
                                ui.label(RichText::new(step_label(step)).strong());
                                ui.label(named_summary(step, &installed_plugins));
                                 if !step.is_disabled() {
                                     if let Err(e) = validate_key(&Key { steps: vec![step.clone()], ..Key::default() }, &validation_config) {
                                         ui.colored_label(Color32::LIGHT_RED, crate::i18n::trf("À corriger : {e}", &[("e", &(e))] ));
                                     }
                                 }
                                let mut capture = false;
                                let mut icon = None;
                                changed |= edit_step(ui, step, &profiles, &pages, &mut capture, &self.audio, &mut icon,
                                    &dialog_tx, &(target.0, target.1.clone(), target.2, path.clone()));
                                if matches!(step.active(), Step::PluginAction { .. }) {
                                    changed |= plugin_fields(ui, step.active_mut(), &validation_config, &installed_plugins, false);
                                }
                                if capture { record = Some(path.clone()); }
                                if let Some(state) = icon { pick_audio = Some((path.clone(), state)); }
                            }
                        });
                    });
                if close { selection = None; }
            }
        }
        if self.workshop_journal.confirm_real {
            egui::Window::new(crate::i18n::tr("Confirmer l'exécution réelle")).collapsible(false).resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                    ui.label(crate::i18n::trf("Ceci va réellement exécuter {count} blocs sur ce PC.", &[("count", &(self.config.keys()[selected].steps.len()))] ));
                    ui.label(crate::i18n::tr("Les actions peuvent modifier les applications, le son et les périphériques."));
                    ui.horizontal(|ui| {
                        if ui.button(crate::i18n::tr("Annuler")).clicked() { self.workshop_journal.confirm_real = false; }
                        if ui.add(theme::danger_button(crate::i18n::tr("Confirmer l'exécution réelle"))).clicked() {
                            self.workshop_journal.confirm_real = false;
                            self.start_workshop_run(false);
                        }
                    });
                });
        }
        self.workshop_chat(ctx, selection.as_deref());
        let highlight = if selected_trace.is_empty() { &self.running_block } else { &selected_trace };
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(RichText::new(crate::i18n::tr("Construis ton automatisation")).size(21.0).strong());
            if self.preferences.workshop_advanced {
                egui::ScrollArea::vertical().id_salt("workflow_advanced_scroll").show(ui, |ui| {
                    changed |= block_editor(ui, &mut self.config.keys_mut()[selected].steps, &profiles, &pages, &self.audio,
                        &[], &mut record, 0, highlight, &mut pick_audio, &dialog_tx, &target, &validation_config, true, &installed_plugins);
                });
            } else {
                egui::ScrollArea::vertical().id_salt("workflow_scroll").show(ui, |ui| {
                    let steps = &self.config.keys()[selected].steps;
                    if steps.is_empty() {
                        ui.add_space(24.0);
                        ui.label(RichText::new(crate::i18n::tr("Ajoute ton premier bloc")).size(20.0).strong());
                        ui.label(crate::i18n::tr("Commence par une action, puis compose ta séquence."));
                        ui.horizontal_wrapped(|ui| {
                            for kind in [0, 2, 5, 39] {
                                if ui.button(block_name(kind)).clicked() { add = Some(kind); }
                            }
                        });
                        if ui.button(crate::i18n::tr("Demander à l'assistant")).clicked() { self.workshop_chat.open = true; }
                    } else {
                        sequence_cards(ui, steps, &[], &mut selection, &mut action, &mut insert_at, &validation_config, &installed_plugins);
                    }
                    ui.menu_button(crate::i18n::tr("+ Ajouter un bloc"), |ui| {
                        for &(category, kinds) in BLOCK_CATEGORIES {
                            ui.menu_button(category_name(category), |ui| {
                                for &kind in kinds {
                                    if !self.preferences.workshop_advanced && kind == 18 { continue; }
                                    if ui.button(block_name(kind)).clicked() { add = Some(kind); ui.close_menu(); }
                                }
                            });
                        }
                        if let Some(step) = plugin_choices(ui, &installed_plugins, "") { add_plugin = Some(step); }
                    });
                });
            }
            if let Err(e) = validate_key(&self.config.keys()[selected], &self.config) {
                ui.colored_label(Color32::LIGHT_RED, crate::i18n::trf("Brouillon : {e}", &[("e", &(e))] ));
            }
        });
        // Un clic sur un bloc doit afficher ses propriétés : en fenêtre compacte, la
        // bibliothèque et l'assistant occupent la même colonne et sont donc refermés.
        if selection.is_some() && selection != selection_before && !self.preferences.workshop_advanced {
            if compact { ctx.data_mut(|d| d.insert_temp(palette_id, false)); }
            self.workshop_chat.open = false;
        }
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
        if let Some(new_step) = add_plugin {
            let mut at = insert_at.take().unwrap_or_else(|| {
                let steps = &self.config.keys()[selected].steps;
                vec![selection.as_ref().and_then(|path| path.first()).map_or(steps.len(), |i| (i + 1).min(steps.len()))]
            });
            if let Some(new_path) = crate::workshop_ops::apply(&mut self.config.keys_mut()[selected].steps,
                crate::workshop_ops::Edit::Insert(std::mem::take(&mut at), new_step)) {
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
                            .add_filter(crate::i18n::tr("Script AutoHotkey v2"), &["ahk"]).save_file() else {return};
                        let result=std::fs::write(path,text);
                        let _=tx.send(Some(Box::new(move |app: &mut App| match result {
                            Ok(())=>app.log(crate::i18n::tr("Script AutoHotkey exporté.").into()),
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
        egui::Window::new(crate::i18n::tr("Windows et raccourcis")).open(&mut open).default_width(510.0).show(ctx,|ui|{
            let mut enabled=self.preferences.start_with_windows;
            if ui.checkbox(&mut enabled,crate::i18n::tr("Lancer NovyDeck au démarrage de Windows")).changed(){startup=Some(enabled);}
            ui.indent("start_minimized",|ui|{
                let mut minimized=self.preferences.start_minimized;
                if ui.add_enabled(self.preferences.start_with_windows,egui::Checkbox::new(&mut minimized,crate::i18n::tr("Démarrer réduit près de l’horloge, sans afficher la fenêtre"))).changed(){
                    self.preferences.start_minimized=minimized;
                    startup=Some(true);
                }
            });
            changed|=ui.checkbox(&mut self.preferences.close_to_tray,crate::i18n::tr("Fermer la fenêtre garde NovyDeck près de l’horloge")).changed();
            if ui.add_enabled(self.tray_ready,egui::Button::new(crate::i18n::tr("Masquer maintenant"))).clicked(){ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));}
            ui.separator();ui.label(crate::i18n::tr("OBS Studio (WebSocket local)"));
            changed |= ui.text_edit_singleline(&mut self.config.obs.host).changed();
            changed |= ui.add(egui::DragValue::new(&mut self.config.obs.port).range(1..=65535)).changed();
            changed |= ui.add(egui::TextEdit::singleline(&mut self.config.obs.password).password(true).hint_text(crate::i18n::tr("Mot de passe OBS"))).changed();
            ui.label(crate::obs_monitor::snapshot().status);
            if ui.button(crate::i18n::tr("Tester la connexion")).clicked() { crate::obs_monitor::update(&self.config.obs, true); }
            ui.separator();ui.label(RichText::new(crate::i18n::trf("Raccourci global · touche {key}", &[("key", &(format!("{:02}",self.selected+1)))] )).strong());
            ui.label(crate::i18n::tr("Actif sur la page actuellement ouverte. Les raccourcis restent disponibles sans périphérique branché."));
            let name=self.config.profiles[self.config.active].name.clone();let page=self.config.page.clone();let key=self.selected;
            let index=self.preferences.triggers.iter().position(|t|t.kind=="hotkey"&&t.profile==name&&t.page==page&&t.key==key);
            let mut text=index.map(|i|self.preferences.triggers[i].keys.clone()).unwrap_or_default();
            ui.horizontal(|ui|{if ui.text_edit_singleline(&mut text).changed(){hotkeys=true;}if ui.button(crate::i18n::tr("Enregistrer…")).clicked(){capture=true;}});
            if !text.is_empty() { match crate::desktop::chord(&text) { Ok(_) => {}, Err(e) => { ui.colored_label(Color32::LIGHT_RED,e.to_string()); } } }
            ui.label(RichText::new(crate::i18n::tr("Exemple : CTRL+ALT+F9. Vide le champ pour désactiver. Windows signale les combinaisons déjà utilisées dans le journal.")).small());
            if hotkeys {if let Some(i)=index {if text.is_empty(){self.preferences.triggers.remove(i);}else{self.preferences.triggers[i].keys=text;}}else if !text.is_empty()&&self.preferences.triggers.len()<256{self.preferences.triggers.push(crate::desktop::Trigger::hotkey(name.clone(),page.clone(),key,text));}changed=true;}
            ui.separator();
            ui.label(RichText::new(crate::i18n::tr("Déclencheurs automatiques")).strong());
            ui.label(crate::i18n::tr("Surveillance toutes les secondes, sans action au démarrage. La pause globale bloque également ces déclencheurs."));
            egui::ComboBox::from_id_salt("automatic_trigger_kind").selected_text(match self.trigger_kind.as_str() {
                "app_open" => crate::i18n::tr("Ouverture d’application"), "app_close" => crate::i18n::tr("Fermeture d’application"),
                "clock" => crate::i18n::tr("Heure fixe"), "timer" => crate::i18n::tr("Minuteur"),
                "audio_connect" => crate::i18n::tr("Branchement audio"), _ => crate::i18n::tr("Débranchement audio"),
            }).show_ui(ui, |ui| {
                for (kind, label) in [("app_open", "Ouverture d’application"), ("app_close", "Fermeture d’application"), ("clock", "Heure fixe"), ("timer", "Minuteur"), ("audio_connect", "Branchement audio"), ("audio_disconnect", "Débranchement audio")] {
                    ui.selectable_value(&mut self.trigger_kind, kind.into(), match kind { "app_open" => crate::i18n::tr("Ouverture d’application"), "app_close" => crate::i18n::tr("Fermeture d’application"), "clock" => crate::i18n::tr("Heure fixe"), "timer" => crate::i18n::tr("Minuteur"), "audio_connect" => crate::i18n::tr("Branchement audio"), _ => crate::i18n::tr("Débranchement audio") });
                }
            });
            match self.trigger_kind.as_str() {
                "timer" => { ui.horizontal(|ui| { ui.label(crate::i18n::tr("Toutes les (minutes)")); ui.add(egui::DragValue::new(&mut self.trigger_minutes).range(1..=1440)); }); },
                "clock" => {
                    ui.horizontal(|ui| { ui.label(crate::i18n::tr("Heure HH:MM")); ui.text_edit_singleline(&mut self.trigger_value); });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(crate::i18n::tr("Jours (aucun = tous) :"));
                        for (d, label) in ["Dim", "Lun", "Mar", "Mer", "Jeu", "Ven", "Sam"].iter().enumerate() {
                            let mut selected = self.trigger_days.contains(&(d as u8));
                            if ui.checkbox(&mut selected, match d { 0 => crate::i18n::tr("Dim"), 1 => crate::i18n::tr("Lun"), 2 => crate::i18n::tr("Mar"), 3 => crate::i18n::tr("Mer"), 4 => crate::i18n::tr("Jeu"), 5 => crate::i18n::tr("Ven"), _ => crate::i18n::tr("Sam") }).changed() {
                                if selected { self.trigger_days.push(d as u8); } else { self.trigger_days.retain(|&n| n != d as u8); }
                            }
                        }
                    });
                },
                _ => {
                    ui.horizontal(|ui| { ui.label(if self.trigger_kind.starts_with("app_") { crate::i18n::tr("Nom .exe") } else { crate::i18n::tr("Nom contient") }); ui.text_edit_singleline(&mut self.trigger_value); });
                    if self.trigger_kind.starts_with("audio_") { ui.checkbox(&mut self.trigger_input, crate::i18n::tr("Périphérique d’entrée (microphone)")); }
                }
            }
            if ui.add(crate::theme::secondary_button(crate::i18n::trf("Ajouter → touche {key} (profil et page actuels)", &[("key", &(format!("{:02}",key+1)))] ))).clicked() {
                let trigger = crate::desktop::Trigger { profile: name.clone(), page: page.clone(), key, keys: String::new(), kind: self.trigger_kind.clone(), value: self.trigger_value.clone(), days: self.trigger_days.clone(), minutes: self.trigger_minutes, input: self.trigger_input };
                if let Err(e) = trigger.validate() { self.fail(crate::i18n::trf("Déclencheur : {e}", &[("e", &(e))] )); }
                else if self.preferences.triggers.len() >= 256 { self.fail(crate::i18n::tr("256 déclencheurs maximum").into()); }
                else { self.preferences.triggers.push(trigger); changed = true; }
            }
            let mut remove = None;
            for (i, trigger) in self.preferences.triggers.iter().enumerate().filter(|(_, t)| t.kind != "hotkey") {
                ui.horizontal(|ui| {
                    ui.label(format!("{} → {} / {} / touche {:02}", trigger.description(), trigger.profile, if trigger.page.is_empty() { crate::i18n::tr("Accueil") } else { &trigger.page }, trigger.key + 1));
                    if ui.add(crate::theme::danger_button(crate::i18n::tr("Supprimer"))).clicked() { remove = Some(i); }
                });
            }
            if let Some(i) = remove { self.preferences.triggers.remove(i); changed = true; }
            if self.trigger_recording{ui.colored_label(accent(),crate::i18n::tr("Appuie sur la combinaison souhaitée…"));if ui.button(crate::i18n::tr("Annuler")).clicked(){self.trigger_recording=false;self.refresh_hotkeys();}}
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
            match crate::desktop::startup(enable, self.preferences.start_minimized) {
                Ok(()) => {
                    self.preferences.start_with_windows = enable;
                    changed = true;
                    self.log(crate::i18n::tr("Démarrage Windows mis à jour.").into());
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
