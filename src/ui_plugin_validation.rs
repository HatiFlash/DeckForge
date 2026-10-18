//! Validation isolée par le traitement réel des événements App ; jamais de périphérique physique.
use super::*;
use crate::{model::PluginBinding, plugin_runtime::Manager, plugins::Plugin};
use serde_json::json;

#[cfg(test)]
const LIMIT: Duration = Duration::from_secs(5);

#[cfg(test)]
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
#[cfg(test)]
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
#[cfg(test)]
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

/// App isolée : même routage Key/KeyRelease que le témoin, sans périphérique ni variables globales.
pub(crate) fn validate_oracle(
    plugin: &Plugin,
    action: &str,
    oracle: &crate::plugin_validation::ActionOracle,
) -> Result<String, String> {
    use crate::plugin_validation::Observation;
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = dir.path().join("config.json");
    let ctx = egui::Context::default();
    let mut app = App::init_with(&ctx, Some(path.clone()), false);
    let (tx, rx) = mpsc::channel();
    app.events = rx;
    app.gate.enabled = true;
    app.gate.connected = true;
    let revision = app.gate.request_apply();
    if !app.gate.applied(revision) {
        return Err("Configuration virtuelle non appliquée".into());
    }
    let settings = if action == "org.deckforge.counter.write_file"
        && plugin.uuid == "org.deckforge.counter"
        && matches!(&plugin.runtime,crate::plugins::Runtime::Node(p) if std::fs::read_to_string(p).ok().is_some_and(|s|s.replace("\r\n","\n")==include_str!("../tests/fixtures/counter.sdPlugin/plugin.js").replace("\r\n","\n")))
    {
        if let crate::plugin_validation::Observation::File {
            directory: Some(folder),
            ..
        } = &oracle.observation
        {
            json!({"output_dir":folder})
        } else {
            json!({})
        }
    } else {
        json!({})
    };
    app.config.keys_mut()[0].plugin = Some(PluginBinding {
        plugin: plugin.uuid.clone(),
        action: action.into(),
        settings,
        state: 0,
    });
    app.applied = app.config.clone();
    store::save(&path, &app.config).map_err(|e| e.to_string())?;
    let file_watch = if let Observation::File {
        directory: Some(folder),
        extension,
    } = &oracle.observation
    {
        let target = std::path::Path::new(folder);
        let canonical = target
            .canonicalize()
            .map_err(|e| format!("Dossier de destination introuvable : {e}"))?;
        let plugin_root = plugin.folder.canonicalize().map_err(|e| e.to_string())?;
        if canonical.starts_with(plugin_root) {
            return Err("Observation interdite dans le bundle".into());
        }
        let snapshot = || -> Result<std::collections::HashSet<std::path::PathBuf>, String> {
            std::fs::read_dir(&canonical)
                .map_err(|e| e.to_string())?
                .map(|entry| entry.map(|e| e.path()).map_err(|e| e.to_string()))
                .collect()
        };
        Some((canonical.clone(), extension.clone(), snapshot()?))
    } else {
        None
    };
    app.plugins = Some(Manager::new(vec![plugin.clone()]));
    app.plugins.as_mut().unwrap().sync(&app.applied);
    let now = Instant::now();
    tx.send(Event::Key(0, now)).map_err(|e| e.to_string())?;
    tx.send(Event::KeyRelease(0, now))
        .map_err(|e| e.to_string())?;
    app.events();
    let deadline = Instant::now() + Duration::from_millis(oracle.timeout_ms);
    loop {
        app.plugin_events();
        if let Some(err) = app.plugin_errors.values().next() {
            return Err(format!("Plugin en erreur : {err}"));
        }
        let context = app
            .plugins
            .as_ref()
            .unwrap()
            .visible()
            .values()
            .find(|v| v.index == 0)
            .map(|v| v.context.clone());
        let face = context.as_ref().and_then(|id| app.plugin_faces.get(id));
        let observed = match &oracle.observation {
            Observation::Title { equals } => face.is_some_and(|f| &f.title == equals),
            Observation::State { equals } => face.is_some_and(|f| f.state == *equals as u8),
            Observation::Image { starts_with } => face
                .and_then(|f| f.image_bytes.as_ref())
                .is_some_and(|bytes| bytes.starts_with(starts_with.as_bytes())),
            Observation::File { .. } => {
                if let Some((folder, extension, before)) = &file_watch {
                    std::fs::read_dir(folder)
                        .map_err(|e| e.to_string())?
                        .filter_map(Result::ok)
                        .any(|entry| {
                            let p = entry.path();
                            !before.contains(&p)
                                && p.is_file()
                                && p.extension()
                                    .and_then(|s| s.to_str())
                                    .is_some_and(|s| s.eq_ignore_ascii_case(extension))
                                && entry
                                    .metadata()
                                    .ok()
                                    .and_then(|m| m.modified().ok())
                                    .is_some_and(|t| {
                                        t >= std::time::SystemTime::now() - Duration::from_secs(15)
                                    })
                        })
                } else {
                    false
                }
            }
            _ => false,
        };
        if observed {
            return Ok(format!(
                "Observation confirmée après appui : {}",
                oracle.expected_effect
            ));
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Délai dépassé ({} ms) : {} non observé",
                oracle.timeout_ms, oracle.expected_effect
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
