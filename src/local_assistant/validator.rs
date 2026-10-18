use super::proposal::{Missing, Proposal, Status, Suggestion};
use crate::model::{validate_key, Config, Key, Step};

fn missing(steps: &[Step], prefix: &[usize], out: &mut Vec<Missing>) {
    for (index, step) in steps.iter().enumerate() {
        let mut path = prefix.to_vec();
        path.push(index);
        let field = match step {
            Step::Launch { program, .. } if program.trim().is_empty() => Some("program"),
            Step::ObsScene { name } if name.trim().is_empty() => Some("name"),
            Step::Hotkey { keys } if keys.trim().is_empty() => Some("keys"),
            Step::Notify { message, .. } if message.trim().is_empty() => Some("message"),
            Step::Profile { name } if name.trim().is_empty() => Some("name"),
            Step::Repeat { steps, .. } if steps.is_empty() => Some("steps"),
            Step::Repeat { steps, .. } => {
                missing(steps, &path, out);
                None
            }
            _ => None,
        };
        if let Some(field) = field {
            out.push(Missing {
                path,
                field,
                reason: format!("{} : à compléter", step.name()),
            });
        }
    }
}

pub fn validate(mut proposal: Proposal, config: &Config) -> Proposal {
    missing(&proposal.steps, &[], &mut proposal.missing);
    // Ne jamais valider une copie artificiellement remplie : les erreurs du modèle
    // ne sont exemptées que pour les champs explicitement signalés ci-dessus.
    if proposal.status == Status::Reconnue {
        let key = Key {
            steps: proposal.steps.clone(),
            ..Key::default()
        };
        if let Err(error) = validate_key(&key, config) {
            let text = error.to_string();
            if proposal.missing.is_empty() {
                proposal.notes.push(format!("Validation : {text}"));
                proposal.status = Status::Ambigue {
                    question: format!("Paramètre invalide : {text}. Que souhaites-tu corriger ?"),
                    choix: vec!["Corriger le bloc".into()],
                };
            } else {
                // Valider séparément tous les blocs complets (la règle de navigation
                // exige aussi la vérification de la séquence entière).
                let clean: Vec<_> = proposal
                    .steps
                    .iter()
                    .filter(|s| !has_missing(s))
                    .cloned()
                    .collect();
                if let Err(e) = validate_key(
                    &Key {
                        steps: clean,
                        ..Key::default()
                    },
                    config,
                ) {
                    proposal.notes.push(format!("Validation : {e}"));
                    proposal.status = Status::Ambigue {
                        question: format!("Séquence invalide : {e}. Que faut-il modifier ?"),
                        choix: vec!["Modifier la séquence".into()],
                    };
                } else {
                    proposal.notes.push(format!(
                        "Validation en attente des champs à compléter : {text}"
                    ));
                }
            }
        }
    }
    proposal
}

fn has_missing(step: &Step) -> bool {
    let mut found = Vec::new();
    missing(std::slice::from_ref(step), &[], &mut found);
    !found.is_empty()
}

pub fn explain(step: &Step) -> (String, Vec<Missing>) {
    let mut fields = Vec::new();
    missing(std::slice::from_ref(step), &[], &mut fields);
    let detail = match step {
        Step::Delay { ms } => format!("Attend {ms} ms avant le bloc suivant."),
        Step::Launch { .. } => {
            "Lance l'application indiquée par un chemin réel ; aucun exécutable n'est deviné."
                .into()
        }
        Step::ObsScene { name } => format!("Active la scène OBS « {name} »."),
        Step::Hotkey { keys } => format!("Envoie le raccourci {keys}."),
        Step::Repeat { count, .. } => format!("Répète les sous-blocs {count} fois."),
        _ => format!(
            "Bloc {} : vérifie ses paramètres avant insertion.",
            step.name()
        ),
    };
    (detail, fields)
}

pub fn diagnose(steps: &[Step]) -> Vec<Suggestion> {
    let mut out = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        if let Step::Delay { ms: 0 } = step {
            let mut fix = steps.to_vec();
            fix.remove(i);
            out.push(Suggestion {
                reason: format!("Attente nulle au bloc {} : supprimer", i + 1),
                steps: fix,
            });
        }
        if let Step::Delay { ms: a } = step {
            if let Some(Step::Delay { ms: b }) = steps.get(i + 1) {
                if let Some(sum) = a.checked_add(*b).filter(|n| *n <= 3_600_000) {
                    let mut fix = steps.to_vec();
                    fix.splice(i..=i + 1, [Step::Delay { ms: sum }]);
                    out.push(Suggestion {
                        reason: "Attentes consécutives : fusionner".into(),
                        steps: fix,
                    });
                }
            }
        }
        let reason = match step {
            Step::Launch { program, .. } if program.trim().is_empty() => {
                "Application sans chemin : renseigner le chemin réel"
            }
            Step::ObsScene { name } if name.trim().is_empty() => {
                "Scène OBS vide : renseigner son nom"
            }
            Step::Hotkey { keys } if keys.trim().is_empty() => {
                "Raccourci vide : saisir des touches"
            }
            Step::Repeat { steps, .. } if steps.is_empty() => {
                "Répéter sans sous-bloc : ajouter un bloc"
            }
            Step::Disabled { .. } => "Bloc désactivé : vérifier s'il doit être réactivé",
            _ => continue,
        };
        // Correctif conservateur : retirer un bloc incomplet, ou proposer
        // explicitement de réactiver un bloc désactivé. Jamais appliqué ici.
        let mut fix = steps.to_vec();
        if let Step::Disabled { step } = step {
            fix[i] = (**step).clone();
        } else {
            fix.remove(i);
        }
        out.push(Suggestion {
            reason: format!("{reason} (correctif proposé : retirer ou réactiver le bloc)"),
            steps: fix,
        });
    }
    out
}
