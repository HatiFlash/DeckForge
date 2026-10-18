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
        Step::Delay { ms } => format!("Fonction : attend avant le bloc suivant.\nParamètre : durée = {ms} ms.\nExemple : 2000 ms laisse 2 secondes à une application pour s’ouvrir."),
        Step::Launch { program, args, focus_if_running } => format!("Fonction : lance une application depuis son chemin réel.\nParamètres : programme = « {program} », {} argument(s), réutilisation de la fenêtre = {focus_if_running}.\nExemple : choisir OBS avec le sélecteur de fichier puis ajouter une attente. Aucun exécutable n'est deviné.", args.len()),
        Step::Media { command } => format!("Fonction : envoie une commande multimédia Windows.\nParamètre : commande = {command}.\nExemple : lecture/pause pour le lecteur multimédia ; pour cibler uniquement le volume d’une application, utilise Volume applicatif."),
        Step::Text { text } => format!("Fonction : saisit du texte dans la fenêtre cible.\nParamètre : texte de {} caractères.\nExemple : insérer une formule de politesse après avoir placé le curseur dans le champ souhaité.", text.chars().count()),
        Step::Url { url } => format!("Fonction : ouvre une adresse avec l’application associée.\nParamètre : adresse = {url}.\nExemple : ouvrir la page d’accueil d’un site dans le navigateur."),
        Step::Notify { title, message } => format!("Fonction : affiche une notification.\nParamètres : titre = « {title} », message = « {message} ».\nExemple : annoncer la fin d’une séquence."),
        Step::WaitWindow { title, timeout_ms, focus } => format!("Fonction : attend une fenêtre.\nParamètres : titre = « {title} », délai maximal = {timeout_ms} ms, activation = {focus}.\nExemple : attendre la fenêtre de l’application avant d’envoyer son raccourci."),
        Step::Profile { name } => format!("Fonction : change le profil actif.\nParamètre : profil = « {name} ».\nExemple : passer au profil de streaming ; le profil doit déjà exister."),
        Step::Page { id } | Step::Folder { id } => format!("Fonction : ouvre une page du deck.\nParamètre : identifiant = « {id} ».\nExemple : passer à la page des commandes audio ; sélectionne une page existante."),
        Step::ObsScene { name } => format!("Fonction : active une scène dans OBS.\nParamètre : nom exact = « {name} ».\nExemple : Gaming. La scène doit exister et la connexion OBS doit être configurée."),
        Step::Hotkey { keys } => format!("Fonction : envoie un raccourci à la fenêtre active.\nParamètre : touches = {keys}.\nExemple : Ctrl+Shift+M ; vérifie le raccourci dans l’application cible avant un test réel."),
        Step::Repeat { count, steps } => format!("Répète {} sous-bloc(s) {count} fois ; ajoute les actions dans RÉPÉTER (ex. attente puis raccourci).", steps.len()),
        Step::AppVolume { target, action, step, level, .. } => format!("Volume applicatif : cible {target}, opération {action}, pas {step} %, niveau {level} %. Choisis l'application ou la fenêtre active (ex. baisser de 10 %)."),
        Step::DeviceVolume { device, input, action, step, level } => format!("Volume {} : périphérique {device}, opération {action}, pas {step} %, niveau {level} %. Exemple : baisser les haut-parleurs de 10 %.", if *input { "microphone" } else { "sortie" }),
        Step::IfCondition { condition, then_steps, else_steps } => format!("Évalue {} ; exécute ALORS ({} blocs) ou SINON ({} blocs). Ajoute par exemple une notification dans une branche.", condition.summary(), then_steps.len(), else_steps.len()),
        Step::IfWindow { title, then_steps, else_steps } => format!("Teste la présence de la fenêtre « {title} » ; ALORS : {} blocs, SINON : {} blocs. Renseigne un titre de fenêtre existant.", then_steps.len(), else_steps.len()),
        Step::IfVariable { name, value, then_steps, else_steps } => format!("Compare la variable {name} à {value} ; ALORS : {} blocs, SINON : {} blocs. Définis d'abord la variable.", then_steps.len(), else_steps.len()),
        Step::Toggle { a, b } => format!("Alterne entre deux séries d'actions à chaque déclenchement : état 1 ({} blocs), état 2 ({} blocs).", a.len(), b.len()),
        Step::ObsMute { input, mode } => format!("OBS : applique « {mode} » à l'entrée audio « {input} ». Vérifie le nom dans OBS."),
        Step::ObsSourceVisibility { scene, source, mode } => format!("OBS : visibilité « {mode} » de la source « {source} » dans la scène « {scene} ». Vérifie ces noms dans OBS."),
        Step::ObsRecord { mode } | Step::ObsStream { mode } | Step::ObsVirtualCam { mode } | Step::ObsReplay { mode } => format!("OBS : {} en mode « {mode} ». Vérifie la connexion OBS avant de tester.", step.name()),
        _ => format!("{} : paramètres actuels à vérifier dans les propriétés de ce bloc ; aucun chemin ou appareil n'est déduit automatiquement.", step.name()),
    };
    (detail, fields)
}

#[cfg(test)]
mod explanation_tests {
    use super::*;

    #[test]
    fn explanations_name_real_parameters_without_inventing_paths() {
        let (launch, missing) = explain(&Step::Launch {
            program: String::new(),
            args: vec![],
            focus_if_running: false,
        });
        assert!(launch.contains("chemin réel"));
        assert_eq!(missing[0].field, "program");
        let (volume, _) = explain(&Step::AppVolume {
            target: "foreground".into(),
            action: "down".into(),
            step: 10,
            level: 50,
            include_children: true,
        });
        assert!(volume.contains("foreground") && volume.contains("10 %"));
        let (branch, _) = explain(&Step::IfWindow {
            title: "OBS".into(),
            then_steps: vec![Step::Delay { ms: 100 }],
            else_steps: vec![],
        });
        assert!(branch.contains("OBS") && branch.contains("ALORS"));
    }
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
