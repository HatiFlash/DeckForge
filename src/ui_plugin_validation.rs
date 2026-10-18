//! Validation isolée par le traitement réel des événements App ; jamais de périphérique physique.
use super::*;
use crate::{model::PluginBinding, plugin_runtime::Manager, plugins::Plugin};
use serde_json::json;

const LIMIT: Duration = Duration::from_secs(5);

fn wait(app: &mut App, index: usize, title: &str, count: i64) -> Result<(), String> {
    let context = app
        .plugins
        .as_ref()
        .unwrap()
        .visible()
        .values()
        .find(|v| v.index == index)
        .ok_or("Contexte absent")?
        .context
        .clone();
    let deadline = Instant::now() + LIMIT;
    loop {
        app.plugin_events(); // didReceiveSettings et setSettings sont traités par App.
        if let Some(error) = app.plugin_errors.values().next() {
            return Err(format!("Plugin en erreur : {error}"));
        }
        let face = app.plugin_faces.get(&context);
        let value = app.config.keys()[index].plugin.as_ref().unwrap().settings["count"].as_i64();
        if face.is_some_and(|f| {
            f.title == title
                && f.state == 1
                && f.image_bytes
                    .as_deref()
                    .is_some_and(|b| b.starts_with(b"<svg"))
                && f.flash.is_some_and(|(ok, _)| ok == (count != 4))
        }) && value == Some(count)
        {
            let saved = store::load(&app.path).map_err(|e| format!("Relecture : {e}"))?;
            if saved.keys()[index].plugin.as_ref().unwrap().settings["count"] != count {
                return Err("Réglages non sauvegardés par l'application".into());
            }
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Délai dépassé : titre {title} et setSettings non observés"
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn press(
    app: &mut App,
    tx: &Sender<Event>,
    index: usize,
    title: &str,
    count: i64,
) -> Result<(), String> {
    let now = Instant::now();
    tx.send(Event::Key(index, now)).map_err(|e| e.to_string())?;
    tx.send(Event::KeyRelease(index, now))
        .map_err(|e| e.to_string())?;
    app.events(); // Même gate, config appliquée et traitement que les vrais boutons.
    wait(app, index, title, count)?;
    std::thread::sleep(Duration::from_millis(100));
    app.plugin_events();
    if app.config.keys()[index].plugin.as_ref().unwrap().settings["count"] != count {
        return Err("keyUp a modifié le compteur".into());
    }
    Ok(())
}
pub(crate) fn validate_witness(plugin: &Plugin, error: Option<&str>) -> Result<(), String> {
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = dir.path().join("config.json");
    let ctx = egui::Context::default();
    let mut app = App::init_with(&ctx, Some(path.clone()), false);
    let (tx, rx) = mpsc::channel();
    app.events = rx;
    app.gate.enabled = true;
    app.gate.connected = true; // Appareil virtuel seulement : App::init isolée n'appelle pas device::spawn.
    let revision = app.gate.request_apply();
    if !app.gate.applied(revision) {
        return Err("Configuration virtuelle non appliquée".into());
    }
    for index in 0..2 {
        app.config.keys_mut()[index].plugin = Some(PluginBinding {
            plugin: plugin.uuid.clone(),
            action: plugin.actions[0].uuid.clone(),
            settings: json!({"step":1,"count":0}),
            state: 0,
        });
    }
    if let Some(mode) = error {
        app.config.keys_mut()[0].plugin.as_mut().unwrap().settings["error"] = json!(mode);
    }
    app.applied = app.config.clone();
    store::save(&path, &app.config).map_err(|e| e.to_string())?;
    app.plugins = Some(Manager::new(vec![plugin.clone()]));
    app.plugins.as_mut().unwrap().sync(&app.applied);
    if error.is_some() {
        return press(&mut app, &tx, 0, "1", 1);
    }
    press(&mut app, &tx, 0, "1", 1)?;
    let deadline = Instant::now() + LIMIT;
    loop {
        app.plugin_events();
        let globals = std::fs::read(path.with_file_name("plugin-settings.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        if globals
            .as_ref()
            .is_some_and(|v| v[&plugin.uuid]["witness"] == "persisted")
        {
            break;
        }
        if Instant::now() >= deadline {
            return Err("Réglages globaux non sauvegardés par App".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    app.config.keys_mut()[0].plugin.as_mut().unwrap().settings["step"] = json!(3);
    app.applied = app.config.clone();
    store::save(&path, &app.config).map_err(|e| e.to_string())?;
    app.plugins.as_mut().unwrap().sync(&app.applied);
    press(&mut app, &tx, 0, "4", 4)?;
    press(&mut app, &tx, 1, "1", 1)?;
    app.config.profiles[app.config.active]
        .pages
        .push(crate::model::Page::new(
            "temoin-page".into(),
            "Page témoin".into(),
            false,
        ));
    app.navigate(crate::model::Step::Page {
        id: "temoin-page".into(),
    });
    app.applied = app.config.clone();
    app.plugins.as_mut().unwrap().sync(&app.applied);
    let deadline = Instant::now() + LIMIT;
    while !app
        .logs
        .iter()
        .any(|line| line.contains("willDisappear:deckforge-0--0"))
    {
        app.plugin_events();
        if Instant::now() >= deadline {
            return Err("willDisappear non reçu au changement de page".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    app.navigate(crate::model::Step::Page { id: String::new() });
    app.applied = app.config.clone();
    app.plugins.as_mut().unwrap().sync(&app.applied);
    app.plugins.take(); // Nouveau processus Node sur la configuration sauvegardée.
    app.config = store::load(&path).map_err(|e| e.to_string())?;
    app.applied = app.config.clone();
    app.plugin_faces.clear();
    app.plugins = Some(Manager::new(vec![plugin.clone()]));
    app.plugins.as_mut().unwrap().sync(&app.applied);
    press(&mut app, &tx, 0, "7", 7)?;
    let deadline = Instant::now() + LIMIT;
    while !app
        .logs
        .iter()
        .any(|line| line.contains("global-read:persisted"))
    {
        app.plugin_events();
        if Instant::now() >= deadline {
            return Err("getGlobalSettings après redémarrage non observé".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}
