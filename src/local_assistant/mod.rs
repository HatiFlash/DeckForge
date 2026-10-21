mod intent_parser;
mod proposal;
mod structured;
mod templates;
mod validator;

#[allow(unused_imports)] // API préparée pour le chat de la phase suivante.
pub use proposal::{Missing, Proposal, Status, Suggestion};
#[allow(unused_imports)] // API destinée au futur chat.
pub use templates::common as common_templates;
#[allow(unused_imports)] // API destinée au futur chat.
pub use validator::{diagnose, explain};

use crate::model::{Config, Step};
use intent_parser as parser;

// Ordre : expressions spécialisées avant les intentions génériques.
const INTENTS: &[(&str, &[&str])] = &[
    (
        "obs_scene",
        &["passe sur", "scene obs", "scene ", "active la scene"],
    ),
    (
        "obs_record",
        &["enregistrement", "enregistrer obs", "record obs"],
    ),
    ("obs_stream", &["stream", "diffusion obs", "diffuse obs"]),
    ("obs_mic", &["micro obs", "micro dans obs"]),
    (
        "app_volume",
        &[
            "volume de l application",
            "volume de l appli",
            "volume appli",
            "volume application active",
        ],
    ),
    (
        "volume",
        &["volume", "son general", "mets en muet", "coupe le son"],
    ),
    ("wait", &["attends", "attendre", "patiente", "pause de", "wait", "sleep", "delay"]),
    (
        "launch",
        &[
            "lance ",
            "lancer ",
            "ouvre l application",
            "ouvre l appli",
            "demarre ",
        ],
    ),
    (
        "url",
        &[
            "ouvre http",
            "ouvre https",
            "ouvrir http",
            "ouvrir https",
            "ouvre le site",
            "ouvre l url",
            "open http",
            "open https",
            "go to http",
            "go to https",
        ],
    ),
    (
        "media",
        &[
            "lecture",
            "pause musique",
            "musique suivante",
            "piste suivante",
            "piste precedente",
            "morceau suivant",
            "morceau precedent",
        ],
    ),
    ("text", &["tape ", "saisis ", "ecris ", "insere le texte", "type ", "write "]),
    ("notify", &["notification", "notifie", "affiche un message"]),
    ("repeat", &["repete ", "rejoue "]),
    ("page_next", &["page suivante", "prochaine page"]),
    ("page_previous", &["page precedente"]),
    ("back", &["retour au dossier"]),
    ("profile", &["profil ", "change de profil"]),
    ("brightness", &["luminosite"]),
    (
        "clipboard",
        &["presse papiers", "copie le texte", "copier le texte"],
    ),
    ("close", &["ferme la fenetre", "fermer la fenetre"]),
];

fn contains_phrase(haystack: &str, phrase: &str) -> bool {
    // Ne pas reconnaître « volume » dans un autre mot.
    haystack.match_indices(phrase).any(|(i, _)| {
        (i == 0 || !haystack.as_bytes()[i - 1].is_ascii_alphanumeric())
            && (phrase.ends_with(' ')
                || haystack
                    .as_bytes()
                    .get(i + phrase.len())
                    .is_none_or(|c| !c.is_ascii_alphanumeric()))
    })
}

fn has(n: &str, words: &[&str]) -> bool {
    words.iter().any(|w| contains_phrase(n, w))
}
fn window_op(op: &str) -> Step {
    Step::WindowOp { target: "foreground".into(), value: String::new(), op: op.into(), x: 0, y: 0, w: 800, h: 600, monitor: 2 }
}
/// Formulations naturelles (français et anglais) des actions Windows courantes, avant les
/// intentions par mots-clés. Retourne None si rien ne correspond.
fn natural(n: &str) -> Option<Step> {
    let windows = has(n, &["fenetre", "fenetres", "window", "windows", "applis", "applications", "programmes"]);
    let all = has(n, &["toutes", "toute", "tous", "tout", "all", "every", "autres", "other", "others"]);
    let except_active = has(n, &["sauf", "excepte", "hormis", "a part", "except", "but", "other", "others", "autres"])
        || has(n, &["celle sur laquelle", "celle ou je suis", "active", "actuelle", "courante", "en cours", "current", "focused"]);
    let close = has(n, &["ferme", "fermer", "fermez", "quitte", "quitter", "close", "kill"]);
    let minimize = has(n, &["reduis", "reduire", "reduit", "minimise", "minimiser", "minimize", "cache", "cacher", "hide"]);
    if windows && all && except_active && close { return Some(window_op("close_others")); }
    if windows && all && except_active && minimize { return Some(window_op("minimize_others")); }
    if (windows && all && minimize) || has(n, &["affiche le bureau", "afficher le bureau", "montre le bureau", "show desktop", "show the desktop"]) {
        return Some(Step::Hotkey { keys: "WIN+D".into() });
    }
    let this_window = windows && !all;
    if this_window && close { return Some(Step::CloseWindow); }
    if this_window && minimize { return Some(window_op("minimize")); }
    if this_window && has(n, &["agrandis", "agrandir", "maximise", "maximiser", "plein ecran", "maximize"]) { return Some(window_op("maximize")); }
    if this_window && has(n, &["restaure", "restaurer", "restore"]) { return Some(window_op("restore")); }
    if this_window && has(n, &["toujours devant", "toujours au premier plan", "always on top", "epingle", "pin"]) { return Some(window_op("toggle_always_on_top")); }
    if this_window && has(n, &["ecran", "moniteur", "screen", "monitor"]) && has(n, &["deplace", "deplacer", "envoie", "bouge", "move", "send"]) {
        let monitor = n.split_whitespace().find_map(|w| w.parse::<u32>().ok()).filter(|m| (1..=16).contains(m)).unwrap_or(2);
        return Some(Step::WindowOp { target: "foreground".into(), value: String::new(), op: "move_to_monitor".into(), x: 0, y: 0, w: 800, h: 600, monitor });
    }
    let shortcut = |keys: &str| Some(Step::Hotkey { keys: keys.into() });
    if has(n, &["capture d ecran", "capture ecran", "screenshot", "copie d ecran", "outil capture"]) { return shortcut("WIN+SHIFT+S"); }
    if has(n, &["verrouille", "verrouiller", "lock the pc", "lock pc", "lock my pc", "lock screen"]) { return shortcut("WIN+L"); }
    if has(n, &["gestionnaire des taches", "gestionnaire de taches", "task manager"]) { return shortcut("CTRL+SHIFT+ESC"); }
    if has(n, &["explorateur", "explorer", "mes fichiers", "file explorer"]) { return shortcut("WIN+E"); }
    if has(n, &["parametres windows", "reglages windows", "windows settings"]) { return shortcut("WIN+I"); }
    if has(n, &["bureau virtuel suivant", "next desktop", "next virtual desktop"]) { return shortcut("CTRL+WIN+RIGHT"); }
    if has(n, &["bureau virtuel precedent", "previous desktop", "previous virtual desktop"]) { return shortcut("CTRL+WIN+LEFT"); }
    if has(n, &["copier", "copie", "copy"]) && !has(n, &["texte", "text", "presse papiers", "clipboard"]) { return shortcut("CTRL+C"); }
    // Navigateur (s'applique à la fenêtre active).
    for (words, key) in [
        (&["rouvre l onglet", "rouvrir l onglet", "reopen the tab", "reopen closed tab"][..], "CTRL+SHIFT+T"),
        (&["navigation privee", "fenetre privee", "private window", "incognito"][..], "CTRL+SHIFT+N"),
        (&["nouvel onglet", "nouveau onglet", "new tab"][..], "CTRL+T"),
        (&["nouvelle fenetre", "new window"][..], "CTRL+N"),
        (&["ferme l onglet", "ferme cet onglet", "fermer l onglet", "close the tab", "close tab"][..], "CTRL+W"),
        (&["recharge la page", "actualise la page", "rafraichis la page", "recharge", "reload", "refresh the page"][..], "F5"),
    ] {
        if has(n, words) { return shortcut(key); }
    }
    for (words, key) in [
        (&["fleche du bas", "fleche bas", "down arrow", "arrow down", "touche bas"][..], "DOWN"),
        (&["fleche du haut", "fleche haut", "up arrow", "arrow up", "touche haut"][..], "UP"),
        (&["fleche de gauche", "fleche gauche", "left arrow", "arrow left"][..], "LEFT"),
        (&["fleche de droite", "fleche droite", "right arrow", "arrow right"][..], "RIGHT"),
        (&["touche tab", "appuie sur tab", "press tab"][..], "TAB"),
        (&["touche espace", "barre d espace", "press space", "spacebar"][..], "SPACE"),
    ] {
        if has(n, words) { return shortcut(key); }
    }
    if has(n, &["mets la musique en pause", "met la musique en pause", "mets en pause", "met en pause", "mets pause", "met pause", "pause la musique", "pause the music", "pause music", "play pause"]) {
        return Some(Step::Media { command: "play_pause".into() });
    }
    if has(n, &["appuie sur entree", "appuyer sur entree", "touche entree", "valide avec entree", "press enter", "hit enter", "press return"]) { return shortcut("ENTER"); }
    if has(n, &["appuie sur echap", "touche echap", "press escape", "press esc", "hit escape"]) { return shortcut("ESC"); }
    if has(n, &["coller", "colle", "paste"]) { return shortcut("CTRL+V"); }
    if has(n, &["annuler", "annule", "undo"]) { return shortcut("CTRL+Z"); }
    if has(n, &["tout selectionner", "selectionne tout", "select all"]) { return shortcut("CTRL+A"); }
    if has(n, &["enregistre le fichier", "sauvegarde le fichier", "save the file", "save file"]) { return shortcut("CTRL+S"); }
    if has(n, &["change de fenetre", "fenetre suivante", "switch window", "alt tab"]) { return shortcut("ALT+TAB"); }
    // Anglais pour les intentions existantes.
    if has(n, &["play pause", "pause the music", "play the music", "resume music"]) { return Some(Step::Media { command: "play_pause".into() }); }
    if has(n, &["next track", "next song", "skip song"]) { return Some(Step::Media { command: "next".into() }); }
    if has(n, &["previous track", "previous song"]) { return Some(Step::Media { command: "previous".into() }); }
    if has(n, &["next page"]) { return Some(Step::PageNext); }
    if has(n, &["previous page"]) { return Some(Step::PagePrevious); }
    None
}

/// Demandes qui correspondent à plusieurs blocs : on reprend le préréglage du catalogue.
fn multi_step(n: &str) -> Option<Vec<Step>> {
    let plain = has(n, &["sans mise en forme", "sans formatage", "sans format", "texte brut", "plain text", "without formatting", "unformatted"]);
    if plain && has(n, &["coller", "colle", "paste"]) {
        return crate::catalog::presets().into_iter().find(|p| p.key.label == "Coller sans mise en forme").map(|p| p.key.steps);
    }
    None
}

fn clarification(question: &str, choices: &[&str]) -> Status {
    Status::Ambigue {
        question: question.into(),
        choix: choices.iter().map(|s| (*s).into()).collect(),
    }
}

fn parse_one(text: &str) -> (Option<Step>, Option<Status>, Vec<String>) {
    let normalized = parser::normalize(text);
    let n = normalized.as_str();
    if let Some(keys) = parser::hotkey(text) {
        return (Some(Step::Hotkey { keys }), None, vec![]);
    }
    if let Some(step) = natural(n) {
        let notes = if matches!(&step, Step::WindowOp { op, .. } if op == "close_others") {
            vec!["Les applications reçoivent une demande de fermeture et peuvent proposer d'enregistrer ; NovyDeck et la fenêtre active restent ouverts.".into()]
        } else { vec![] };
        return (Some(step), None, notes);
    }
    let kind = INTENTS
        .iter()
        .find(|(_, aliases)| aliases.iter().any(|alias| contains_phrase(n, alias)))
        .map(|(kind, _)| *kind);
    let name = || {
        parser::quoted_or_after(
            text,
            &[
                " sur la scène ",
                " sur la scene ",
                " sur ",
                " scène ",
                " scene ",
            ],
        )
    };
    let step = match kind {
        Some("obs_scene") => Step::ObsScene {
            name: name().unwrap_or_default(),
        },
        Some("obs_record") => Step::ObsRecord { mode: obs_mode(n) },
        Some("obs_stream") => Step::ObsStream { mode: obs_mode(n) },
        Some("obs_mic") => Step::ObsMute {
            input: "Micro/Aux".into(),
            mode: if n.contains("active") || n.contains("retabl") {
                "unmute"
            } else if n.contains("coupe") || n.contains("muet") {
                "mute"
            } else {
                "toggle"
            }
            .into(),
        },
        Some("volume" | "app_volume") => {
            let explicit_app = kind == Some("app_volume")
                || n.contains("application active")
                || n.contains("appli active");
            let explicit_general = n.contains("general") || n.contains("systeme");
            if !explicit_app && !explicit_general {
                return (
                    None,
                    Some(clarification(
                        "Volume de quoi : général ou application active ?",
                        &["Général", "Application active"],
                    )),
                    vec![],
                );
            }
            let down = ["baisse", "diminue", "reduis"]
                .iter()
                .any(|w| n.contains(w));
            let up = ["augmente", "monte"].iter().any(|w| n.contains(w));
            let mute = n.contains("muet") || n.contains("coupe");
            let action = if mute {
                "mute"
            } else if down {
                "down"
            } else if up {
                "up"
            } else {
                "set"
            };
            let pct = parser::percent(text);
            if !mute && pct.is_none() {
                return (
                    None,
                    Some(clarification(
                        "De combien de pour cent faut-il régler le volume ?",
                        &["Indiquer un pourcentage entre 1 et 100"],
                    )),
                    vec![],
                );
            }
            let p = pct.unwrap_or(5);
            if explicit_app {
                Step::AppVolume {
                    target: "foreground".into(),
                    action: action.into(),
                    step: p,
                    level: p,
                    include_children: true,
                }
            } else {
                Step::DeviceVolume {
                    input: false,
                    device: "default".into(),
                    action: action.into(),
                    step: p,
                    level: p,
                }
            }
        }
        Some("wait") => match parser::duration(text) {
            Some(ms) => Step::Delay { ms },
            None => {
                return (
                    None,
                    Some(clarification(
                        "Combien de temps faut-il attendre (ms, secondes ou minutes) ?",
                        &["Indiquer une durée"],
                    )),
                    vec![],
                )
            }
        },
        Some("launch") => Step::Launch {
            program: String::new(),
            args: vec![],
            focus_if_running: false,
        },
        Some("url") => {
            let url = text
                .split_whitespace()
                .find(|s| s.starts_with("http://") || s.starts_with("https://"));
            match url {
                Some(url) => Step::Url {
                    url: url.trim_end_matches(['.', ',', '!']).into(),
                },
                None => {
                    return (
                        None,
                        Some(clarification(
                            "Quelle adresse HTTP ou HTTPS ouvrir ?",
                            &["Indiquer l'URL complète"],
                        )),
                        vec![],
                    )
                }
            }
        }
        Some("media") => {
            let command = if n.contains("suivant") {
                "next"
            } else if n.contains("precedent") {
                "previous"
            } else if n.contains("arrete") {
                "stop"
            } else {
                "play_pause"
            };
            Step::Media {
                command: command.into(),
            }
        }
        Some("text") => Step::Text {
            text: parser::quoted_or_after(
                text,
                &["tape ", "saisis ", "écris ", "ecris ", "texte ", "type ", "write "],
            )
            .unwrap_or_default(),
        },
        Some("notify") => Step::Notify {
            title: "NovyDeck".into(),
            message: parser::quoted_or_after(text, &["message ", "notification ", "notifie "])
                .unwrap_or_default(),
        },
        Some("repeat") => match parser::number(text) {
            Some(count) if (1..=1000).contains(&count) => Step::Repeat {
                count,
                steps: vec![],
            },
            _ => {
                return (
                    None,
                    Some(clarification(
                        "Combien de fois répéter (1 à 1000) ?",
                        &["Indiquer le nombre"],
                    )),
                    vec![],
                )
            }
        },
        Some("page_next") => Step::PageNext,
        Some("page_previous") => Step::PagePrevious,
        Some("back") => Step::Back,
        Some("profile") => Step::Profile {
            name: parser::quoted_or_after(text, &["profil "]).unwrap_or_default(),
        },
        Some("brightness") => {
            let mode = if n.contains("baisse") || n.contains("diminue") {
                "down"
            } else if n.contains("augmente") || n.contains("monte") {
                "up"
            } else {
                "set"
            };
            match parser::percent(text) {
                Some(value) => Step::D6Brightness {
                    mode: mode.into(),
                    value,
                },
                None => {
                    return (
                        None,
                        Some(clarification(
                            "Quelle variation de luminosité en pour cent ?",
                            &["Indiquer un pourcentage"],
                        )),
                        vec![],
                    )
                }
            }
        }
        Some("clipboard") => {
            if n.contains("efface") || n.contains("vide") {
                Step::Clipboard {
                    op: "clear".into(),
                    value: String::new(),
                    name: "texte".into(),
                }
            } else {
                Step::Clipboard {
                    op: "copy_text".into(),
                    value: parser::quoted_or_after(text, &["texte "]).unwrap_or_default(),
                    name: "texte".into(),
                }
            }
        }
        Some("close") => Step::CloseWindow,
        _ => return (None, Some(Status::Inconnue), vec![]),
    };
    let notes = if kind == Some("launch") {
        vec![format!(
            "Application « {} » : indique son chemin exact ; aucun .exe n'est inventé.",
            parser::quoted_or_after(
                text,
                &["lance ", "lancer ", "demarre ", "application ", "appli "]
            )
            .unwrap_or_default()
        )]
    } else {
        vec![]
    };
    (Some(step), None, notes)
}

fn obs_mode(n: &str) -> String {
    if n.contains("arrete") || n.contains("stop") {
        "stop"
    } else if n.contains("demarre") || n.contains("lance") || n.contains("commence") {
        "start"
    } else if n.contains("pause") {
        "pause"
    } else {
        "toggle"
    }
    .into()
}

/// Proposition issue de l'IA locale : mêmes contrôles que les règles (champs manquants, profils…).
pub fn from_model(steps: Vec<Step>, config: &Config) -> Proposal {
    let status = if steps.is_empty() { Status::Inconnue } else { Status::Reconnue };
    validator::validate(
        Proposal {
            steps,
            notes: vec!["Proposé par l'IA locale : vérifie chaque bloc avant d'ajouter.".into()],
            missing: vec![],
            status,
        },
        config,
    )
}

/// Analyse pure : aucune exécution ni mutation de la configuration.
pub fn propose(text: &str, config: &Config) -> Proposal {
    propose_with(text, config, &crate::local_llm::Knowledge::builtin())
}

const OPEN_VERBS: &[&str] = &["ouvre", "ouvrir", "lance", "lancer", "demarre", "demarrer", "open", "launch", "start"];

/// La clause ne contient que « ouvre » + le nom de l'application (« démarre l'enregistrement OBS » n'en est pas une).
fn only_opens(n: &str, app: &crate::local_llm::KnownApp) -> bool {
    let names: Vec<String> = app.noms.iter().map(|name| parser::normalize(name)).chain(std::iter::once(parser::normalize(&app.exe))).collect();
    let mut rest = format!(" {n} ");
    for name in names.iter().filter(|n| !n.is_empty()) {
        rest = rest.replace(&format!(" {name} "), " ");
    }
    rest.split_whitespace().all(|w| OPEN_VERBS.contains(&w) || ["le", "la", "l", "les", "un", "une", "moi", "stp", "please", "the", "app", "application", "appli", "logiciel", "programme", "navigateur", "exe"].contains(&w))
}

/// Comme [`propose`], avec les applications connues du fichier `ia-locale.json`.
/// Une demande conditionnelle ou répétée est soit comprise en entier, soit laissée à l'IA
/// locale (statut inconnu) : jamais de proposition partielle.
pub fn propose_with(text: &str, config: &Config, knowledge: &crate::local_llm::Knowledge) -> Proposal {
    let n = parser::normalize(text);
    let rules = |t: &str| {
        let p = propose_plain(t, config, knowledge);
        matches!(p.status, Status::Reconnue).then_some(p.steps)
    };
    let structured = if structured::is_conditional(&n) {
        Some(structured::condition(text, knowledge, &rules))
    } else if structured::is_repetition(&n) {
        // « répète 3 fois » seul : les règles ordinaires créent la répétition et demandent quoi répéter.
        structured::repetition(text, &rules).map(|step| Some((step, "Répétition : vérifie le nombre de tours avant d'ajouter.".to_owned())))
    } else {
        None
    };
    match structured {
        Some(Some((step, note))) => validator::validate(
            Proposal { steps: vec![step], notes: vec![note], missing: vec![], status: Status::Reconnue },
            config,
        ),
        Some(None) => Proposal {
            steps: vec![],
            notes: vec!["Demande avec condition ou répétition non comprise par les règles : l'IA locale peut la traduire (ou reformule : « si Brave est ouvert, … sinon … », « appuie 3 fois sur … »).".into()],
            missing: vec![],
            status: Status::Inconnue,
        },
        None => propose_plain(text, config, knowledge),
    }
}

fn propose_plain(text: &str, config: &Config, knowledge: &crate::local_llm::Knowledge) -> Proposal {
    let mut result = Proposal {
        steps: vec![],
        notes: vec![],
        missing: vec![],
        status: Status::Inconnue,
    };
    for clause in parser::clauses(text) {
        let cn = parser::normalize(&clause);
        if let Some(steps) = multi_step(&cn) {
            result.steps.extend(steps);
            continue;
        }
        // « mets Edge au premier plan » : fenêtre de l'application connue.
        if has(&cn, &["premier plan", "en avant", "bascule sur", "bascule vers", "passe sur", "switch to", "bring", "focus"]) && !has(&cn, &["toujours", "always"]) {
            if let Some(app) = knowledge.find_app(&clause) {
                result.steps.push(Step::WindowOp { target: "exe".into(), value: app.exe.clone(), op: "activate".into(), x: 0, y: 0, w: 800, h: 600, monitor: 2 });
                continue;
            }
        }
        // « ouvre Brave » : programme tiré des applications connues, jamais inventé.
        if has(&cn, OPEN_VERBS) {
            if let Some(app) = knowledge.find_app(&clause).filter(|app| only_opens(&cn, app)) {
                result.steps.push(Step::Launch { program: app.exe.clone(), args: vec![], focus_if_running: false });
                result.notes.push(format!("Programme : {} (applications connues de ia-locale.json).", app.exe));
                continue;
            }
        }
        let (step, status, notes) = parse_one(&clause);
        if let Some(status) = status {
            result.status = status;
            result.notes.push(format!("Clause non résolue : {clause}"));
            return result;
        }
        if let Some(step) = step {
            result.steps.push(step);
            result.notes.extend(notes);
        }
    }
    if !result.steps.is_empty() {
        result.status = Status::Reconnue;
    }
    validator::validate(result, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn first(text: &str) -> Proposal {
        propose(text, &Config::default())
    }
    fn recognized(text: &str) -> Step {
        let p = first(text);
        assert_eq!(p.status, Status::Reconnue, "{text}: {:?}", p.notes);
        p.steps[0].clone()
    }
    #[test]
    fn natural_french_and_english_window_requests() {
        for text in [
            "fermer toute les fenetres sauf celle sur laquelle je suis",
            "Ferme toutes les fenêtres sauf l'active",
            "ferme les autres fenêtres",
            "close all windows except the current one",
            "close all other windows",
        ] {
            assert!(matches!(recognized(text), Step::WindowOp { op, target, .. } if op == "close_others" && target == "foreground"), "{text}");
        }
        assert!(matches!(recognized("réduis toutes les fenêtres sauf celle-ci active"), Step::WindowOp { op, .. } if op == "minimize_others"));
        assert!(matches!(recognized("minimise toutes les fenêtres"), Step::Hotkey { keys } if keys == "WIN+D"));
        assert!(matches!(recognized("affiche le bureau"), Step::Hotkey { keys } if keys == "WIN+D"));
        assert!(matches!(recognized("agrandis la fenêtre"), Step::WindowOp { op, .. } if op == "maximize"));
        assert!(matches!(recognized("déplace la fenêtre sur l'écran 3"), Step::WindowOp { op, monitor: 3, .. } if op == "move_to_monitor"));
        assert!(matches!(recognized("ferme la fenêtre"), Step::CloseWindow));
        assert!(matches!(recognized("fais une capture d'écran"), Step::Hotkey { keys } if keys == "WIN+SHIFT+S"));
        assert!(matches!(recognized("ouvre le gestionnaire des tâches"), Step::Hotkey { keys } if keys == "CTRL+SHIFT+ESC"));
        let p = first("wait 2 seconds then open https://example.com and press play pause");
        assert_eq!(p.status, Status::Reconnue, "{:?}", p.notes);
        assert!(matches!(&p.steps[..], [Step::Delay { ms: 2000 }, Step::Url { url }, Step::Media { command }] if url == "https://example.com" && command == "play_pause"), "{:?}", p.steps);
        // Les validations existantes restent : le volume ambigu demande toujours une précision.
        assert!(matches!(first("baisse le volume de 10 %").status, Status::Ambigue { .. }));
    }
    #[test]
    fn plain_paste_half_second_and_enter() {
        let p = first("colle sans mise en forme");
        assert_eq!(p.status, Status::Reconnue);
        assert!(matches!(&p.steps[..], [Step::Clipboard { op, .. }, Step::Clipboard { .. }, Step::Hotkey { keys }] if op == "read_to_variable" && keys == "CTRL+V"), "{:?}", p.steps);
        let p = first("wait half a second then press enter");
        assert_eq!(p.status, Status::Reconnue, "{:?}", p.notes);
        assert!(matches!(&p.steps[..], [Step::Delay { ms: 500 }, Step::Hotkey { keys }] if keys == "ENTER"), "{:?}", p.steps);
        assert!(matches!(recognized("attends une demi-seconde"), Step::Delay { ms: 500 }));
        assert!(matches!(recognized("attends deux secondes"), Step::Delay { ms: 2000 }));
        assert!(matches!(recognized("wait one second"), Step::Delay { ms: 1000 }));
        assert!(matches!(recognized("colle"), Step::Hotkey { keys } if keys == "CTRL+V"));
    }
    #[test]
    fn example_sequence() {
        let p = first("Lance OBS, attends 2 s, passe sur Gaming");
        assert_eq!(p.status, Status::Reconnue);
        assert!(
            matches!(&p.steps[..], [Step::Launch { program, .. }, Step::Delay { ms: 2000 }, Step::ObsScene { name }] if program == "obs64.exe" && name == "Gaming")
        );
        // OBS est une application connue : programme rempli, plus rien à compléter.
        assert!(p.missing.is_empty() && p.insertable());
    }
    #[test]
    fn volume_ambigu() {
        let p = first("baisse le volume de 10 %");
        assert!(matches!(p.status, Status::Ambigue { .. }));
        assert!(!p.insertable());
    }
    #[test]
    fn volume_explicit() {
        assert!(matches!(
            recognized("baisse le volume général de 10 %"),
            Step::DeviceVolume { step: 10, .. }
        ));
        assert!(matches!(
            recognized("baisse le volume de l'application active de 10 %"),
            Step::AppVolume { step: 10, .. }
        ));
    }
    #[test]
    fn hotkeys() {
        assert!(
            matches!(recognized("Ctrl+Shift+M"), Step::Hotkey { keys } if keys == "CTRL+SHIFT+M")
        );
        assert!(matches!(recognized("alt f4"), Step::Hotkey { .. }));
        assert!(matches!(recognized("windows+d"), Step::Hotkey { keys } if keys == "WIN+D"));
    }
    #[test]
    fn delays() {
        assert!(matches!(
            recognized("attends 500 ms"),
            Step::Delay { ms: 500 }
        ));
        assert!(matches!(
            recognized("attends 1,5 seconde"),
            Step::Delay { ms: 1500 }
        ));
        assert!(matches!(
            recognized("attends 2 minutes"),
            Step::Delay { ms: 120000 }
        ));
    }
    #[test]
    fn launch_requires_path() {
        let p = first("ouvre l'application MonOutilMaison");
        assert!(matches!(p.steps[0], Step::Launch { .. }));
        assert_eq!(p.missing[0].field, "program");
    }
    #[test]
    fn obs() {
        assert!(
            matches!(recognized("passe sur Gaming"), Step::ObsScene { name } if name == "Gaming")
        );
        assert!(matches!(
            recognized("démarre l'enregistrement OBS"),
            Step::ObsRecord { .. }
        ));
        assert!(matches!(
            recognized("démarre le stream OBS"),
            Step::ObsStream { .. }
        ));
        assert!(matches!(
            recognized("coupe le micro OBS"),
            Step::ObsMute { .. }
        ));
    }
    #[test]
    fn media() {
        assert!(matches!(recognized("lecture pause"), Step::Media { .. }));
        assert!(
            matches!(recognized("piste suivante"), Step::Media { command } if command == "next")
        );
        assert!(
            matches!(recognized("piste précédente"), Step::Media { command } if command == "previous")
        );
    }
    #[test]
    fn text_url_notify() {
        assert!(matches!(recognized("tape \"Bonjour\""), Step::Text { text } if text == "Bonjour"));
        assert!(matches!(
            recognized("ouvre https://example.org"),
            Step::Url { .. }
        ));
        assert!(
            matches!(recognized("notification \"Salut\""), Step::Notify { message, .. } if message == "Salut")
        );
    }
    #[test]
    fn repeat_and_navigation() {
        let p = first("répète 3 fois");
        assert!(matches!(p.steps[0], Step::Repeat { count: 3, .. }));
        assert_eq!(p.missing[0].field, "steps");
        assert!(matches!(recognized("page suivante"), Step::PageNext));
        assert!(matches!(recognized("page précédente"), Step::PagePrevious));
        assert!(matches!(recognized("retour au dossier"), Step::Back));
    }
    #[test]
    fn brightness_clipboard_close() {
        assert!(matches!(
            recognized("baisse la luminosité de 20 %"),
            Step::D6Brightness { value: 20, .. }
        ));
        assert!(
            matches!(recognized("efface le presse-papiers"), Step::Clipboard { op, .. } if op == "clear")
        );
        assert!(matches!(recognized("ferme la fenêtre"), Step::CloseWindow));
    }
    #[test]
    fn unknown_and_invalid() {
        assert_eq!(first("dessine une licorne").status, Status::Inconnue);
        assert!(matches!(
            first("attends 99 heures").status,
            Status::Ambigue { .. }
        ));
        assert!(matches!(first("passe sur").status, Status::Reconnue));
    }
    #[test]
    fn diagnosis_and_explanation() {
        let steps = vec![
            Step::Delay { ms: 100 },
            Step::Delay { ms: 200 },
            Step::Delay { ms: 0 },
            Step::Disabled {
                step: Box::new(Step::CloseWindow),
            },
        ];
        let suggestions = diagnose(&steps);
        assert!(suggestions
            .iter()
            .any(|s| matches!(s.steps.first(), Some(Step::Delay { ms: 300 }))));
        assert!(suggestions.iter().any(|s| s.reason.contains("désactivé")));
        assert!(explain(&Step::Launch {
            program: String::new(),
            args: vec![],
            focus_if_running: false
        })
        .1
        .iter()
        .any(|m| m.field == "program"));
    }
    #[test]
    fn profile_validation() {
        assert!(
            matches!(recognized("change de profil Bureau"), Step::Profile { name } if name == "Bureau")
        );
        assert!(matches!(
            first("change de profil Inexistant").status,
            Status::Ambigue { .. }
        ));
    }
    #[test]
    fn invalid_pct_not_insertable() {
        let p = first("baisse le volume général de 0 %");
        assert!(!p.insertable());
        assert!(matches!(p.status, Status::Ambigue { .. }));
    }
    #[test]
    fn clauses_keep_decimal_and_quotes() {
        let p = first("attends 1,5 seconde, tape \"Bonjour, et merci\"");
        assert_eq!(p.steps.len(), 2);
        assert!(matches!(&p.steps[1], Step::Text { text } if text == "Bonjour, et merci"));
    }
    #[test]
    fn clipboard_copy() {
        assert!(
            matches!(recognized("copie le texte \"salut\" dans le presse-papiers"), Step::Clipboard { value, .. } if value == "salut")
        );
    }
    #[test]
    fn navigation_mixed_is_not_insertable() {
        let p = first("page suivante, attends 2 s");
        assert!(!p.insertable());
        assert!(matches!(p.status, Status::Ambigue { .. }));
    }
    #[test]
    fn templates_reuse_catalog() {
        assert!(!common_templates().is_empty());
    }
    #[test]
    fn no_silent_partial() {
        let p = first("attends 2 s, dessine une licorne");
        assert_eq!(p.status, Status::Inconnue);
        assert!(!p.insertable());
    }
}
