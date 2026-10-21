//! About dialog helpers. Update requests run outside the UI thread.
use super::App;
use eframe::egui::{self, RichText};
use std::{path::PathBuf, process::Command};

pub(super) enum UpdateEvent {
    Checked(Result<crate::app_update::Status, String>),
    Verifying,
    Downloaded(Result<PathBuf, String>),
}

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
pub(super) fn information() -> String {
    format!("NovyDeck {}\nCompilation : {} ({})", env!("CARGO_PKG_VERSION"), HASH, DATE)
}
