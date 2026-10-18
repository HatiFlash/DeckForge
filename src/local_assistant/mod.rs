mod intent_parser;
mod proposal;
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
    ("wait", &["attends", "attendre", "patiente", "pause de"]),
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
    ("text", &["tape ", "saisis ", "ecris ", "insere le texte"]),
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
                &["tape ", "saisis ", "écris ", "ecris ", "texte "],
            )
            .unwrap_or_default(),
        },
        Some("notify") => Step::Notify {
            title: "DeckForge".into(),
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

/// Analyse pure : aucune exécution ni mutation de la configuration.
pub fn propose(text: &str, config: &Config) -> Proposal {
    let mut result = Proposal {
        steps: vec![],
        notes: vec![],
        missing: vec![],
        status: Status::Inconnue,
    };
    for clause in parser::clauses(text) {
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
    fn example_sequence() {
        let p = first("Lance OBS, attends 2 s, passe sur Gaming");
        assert_eq!(p.status, Status::Reconnue);
        assert!(
            matches!(&p.steps[..], [Step::Launch { program, .. }, Step::Delay { ms: 2000 }, Step::ObsScene { name }] if program.is_empty() && name == "Gaming")
        );
        assert_eq!(p.missing[0].field, "program");
        assert_eq!(p.missing[0].path, vec![0]);
        // Insertable as a draft: the path stays empty for the user to fill in.
        assert!(p.insertable());
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
        let p = first("ouvre l'application OBS");
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
