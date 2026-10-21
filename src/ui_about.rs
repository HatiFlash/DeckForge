//! About dialog and private-repository update check. No network activity on the UI thread.
use super::App;
use eframe::egui::{self, RichText};
use std::{process::{Command, Stdio}, thread, time::{Duration, Instant}};

pub(super) fn hardware_report_button(ui: &mut egui::Ui, app: &mut App) {
    if ui.button("Signaler un problème avec mon deck").clicked() { app.open_hardware_report(); }
}

pub(super) fn hardware_report_window(ctx: &egui::Context, app: &mut App) {
    let Some(report) = app.hardware_report.as_ref() else { return; };
    let mut preview = report.text.clone();
    let url = report.issue_url.clone();
    let mut open = true;
    let mut save = false;
    let mut cancel = false;
    egui::Window::new("Signaler un problème avec mon deck")
        .open(&mut open).collapsible(false).resizable(true).default_width(650.0)
        .show(ctx, |ui| {
            ui.label("Vérifie l’aperçu avant de continuer. Le rapport reste sur ton PC : rien n’est envoyé automatiquement. Indique la marque, le modèle exact et le problème sur GitHub, puis glisse le fichier texte dans l’issue.");
            ui.label(RichText::new("APERÇU DU RAPPORT ANONYMISÉ").strong());
            egui::ScrollArea::vertical().max_height(330.0).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut preview).desired_width(f32::INFINITY).interactive(false));
            });
            ui.horizontal(|ui| {
                if ui.button("Enregistrer le rapport et ouvrir GitHub").clicked() { save = true; }
                if ui.button("Annuler").clicked() { cancel = true; }
            });
        });
    if save {
        match crate::hardware_report::save(&app.path, app.hardware_report.as_ref().unwrap()) {
            Ok(path) => {
                // /select, must be a single argument to explorer.exe, never a shell command.
                let selection = format!("/select,{}", path.display());
                if let Err(e) = Command::new("explorer.exe").arg(selection).spawn() {
                    app.fail(format!("Rapport enregistré, sélection dans l’Explorateur impossible : {e}"));
                } else { app.log(format!("Rapport enregistré : {}", path.display())); }
                ctx.open_url(egui::OpenUrl::new_tab(url));
                app.hardware_report = None;
            }
            Err(e) => app.fail(format!("Enregistrement du rapport impossible : {e}")),
        }
    } else if cancel || !open { app.hardware_report = None; }
}


pub(super) const HASH: &str = env!("DECKFORGE_GIT_HASH");
pub(super) const DATE: &str = env!("DECKFORGE_BUILD_DATE");
const REPOSITORY: &str = "repos/HatiFlash/NovyDeck";

pub(super) fn information() -> String {
    format!("NovyDeck {}\nCompilation : {} ({})", env!("CARGO_PKG_VERSION"), HASH, DATE)
}

// Pure decision logic: never equate a failed or malformed response to an up-to-date build.
pub(super) fn decide(hash: &str, latest: Result<&str, &str>, comparison: Result<&str, &str>) -> String {
    if hash == "inconnu" || hash.len() < 7 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return "Compilation sans identifiant".into();
    }
    let latest = match latest {
        Ok(value) if value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()) => value,
        Ok(_) => return "Réponse GitHub invalide".into(),
        Err(error) => return error.into(),
    };
    if latest.starts_with(hash) {
        return "À jour : cette compilation correspond à la dernière version sur GitHub".into();
    }
    let comparison = match comparison {
        Ok(value) => value.trim(),
        Err(error) => return error.into(),
    };
    let mut fields = comparison.split_whitespace();
    match (fields.next(), fields.next(), fields.next()) {
        (Some("ahead"), Some(n), None) if n.parse::<u64>().is_ok_and(|n| n > 0) =>
            format!("Nouvelle version disponible : {n} modification(s) sur GitHub"),
        (Some("behind"), Some(n), None) if n.parse::<u64>().is_ok() =>
            "Cette compilation est plus récente que GitHub".into(),
        (Some("diverged"), Some(n), None) if n.parse::<u64>().is_ok() =>
            "Versions différentes".into(),
        _ => "Réponse GitHub invalide".into(),
    }
}

fn gh(args: &[&str], deadline: Instant) -> Result<String, &'static str> {
    use std::os::windows::process::CommandExt;
    let mut child = Command::new("gh")
        .args(args).creation_flags(0x0800_0000)
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e| if e.kind() == std::io::ErrorKind::NotFound {
            "GitHub CLI absent"
        } else { "Réseau indisponible ou délai dépassé" })?;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().map_err(|_| "Réseau indisponible ou délai dépassé")?;
                if !status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
                    // Never expose stderr: it may contain repository details or credentials.
                    return Err(if stderr.contains("authentication") || stderr.contains("authenticate")
                        || stderr.contains("not logged") || stderr.contains("http 401")
                        || stderr.contains("http 403") || stderr.contains("http 404")
                        || stderr.contains("not found") || stderr.contains("login") {
                        "GitHub CLI non connecté (dépôt privé)"
                    } else { "Réseau indisponible ou délai dépassé" });
                }
                return String::from_utf8(output.stdout).map_err(|_| "Réponse GitHub invalide");
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Réseau indisponible ou délai dépassé");
            }
        }
    }
}

pub(super) fn check() -> String {
    if HASH == "inconnu" || HASH.len() < 7 || !HASH.bytes().all(|b| b.is_ascii_hexdigit()) {
        return "Compilation sans identifiant".into();
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    let latest = gh(&["api", &format!("{REPOSITORY}/commits/main"), "--jq", ".sha"], deadline);
    let latest = match latest {
        Ok(value) => value,
        Err(error) => return error.into(),
    };
    if latest.trim().starts_with(HASH) {
        return decide(HASH, Ok(latest.trim()), Ok(""));
    }
    let compare = gh(&["api", &format!("{REPOSITORY}/compare/{HASH}...main"),
        "--jq", ".status + \" \" + (.ahead_by|tostring)"], deadline);
    decide(HASH, Ok(latest.trim()), compare.as_deref().map_err(|error| *error))
}

#[cfg(test)]
mod tests {
    use super::*;
    const LOCAL: &str = "4f3cab7";
    const REMOTE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    #[test]
    fn update_decisions_offline() {
        assert!(decide(LOCAL, Ok("4f3cab7aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), Ok("" )).starts_with("À jour"));
        assert_eq!(decide(LOCAL, Ok(REMOTE), Ok("ahead 3")), "Nouvelle version disponible : 3 modification(s) sur GitHub");
        assert_eq!(decide(LOCAL, Ok(REMOTE), Ok("behind 2")), "Cette compilation est plus récente que GitHub");
        assert_eq!(decide(LOCAL, Ok(REMOTE), Ok("diverged 1")), "Versions différentes");
        assert_eq!(decide("inconnu", Ok(REMOTE), Ok("ahead 1")), "Compilation sans identifiant");
        assert_eq!(decide(LOCAL, Err("GitHub CLI absent"), Ok("ahead 1")), "GitHub CLI absent");
        assert_eq!(decide(LOCAL, Err("GitHub CLI non connecté (dépôt privé)"), Ok("ahead 1")), "GitHub CLI non connecté (dépôt privé)");
        assert_eq!(decide(LOCAL, Ok(REMOTE), Err("Réseau indisponible ou délai dépassé")), "Réseau indisponible ou délai dépassé");
        assert_eq!(decide(LOCAL, Ok("bad"), Ok("ahead 1")), "Réponse GitHub invalide");
        assert_eq!(decide(LOCAL, Ok(REMOTE), Ok("ahead 0")), "Réponse GitHub invalide");
    }
}
