//! Virtual-device validation: same Manager::key routing as Event::Key/KeyRelease, never USB.
use crate::{
    model::{Config, PluginBinding},
    plugin_runtime::{Manager, Reply},
    plugins::{Plugin, Runtime},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const WITNESS: &str = "org.deckforge.counter";
const ACTION: &str = "org.deckforge.counter.increment";
const LIMIT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Passed,
    Failed,
    Unverified,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionReport {
    pub action: String,
    pub verdict: Verdict,
    pub reason: String,
    pub mode: String,
    pub tested_at_unix: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub plugin: String,
    pub version: String,
    pub actions: Vec<ActionReport>,
}

fn stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn trusted(plugin: &Plugin) -> bool {
    plugin.uuid == WITNESS
        && plugin.actions.len() == 1
        && plugin.actions[0].uuid == ACTION
        && matches!(&plugin.runtime, Runtime::Node(path) if std::fs::read_to_string(path).ok()
            .is_some_and(|s| s.replace("\r\n", "\n") == include_str!("../tests/fixtures/counter.sdPlugin/plugin.js").replace("\r\n", "\n")))
}
pub fn simulate(plugin: &Plugin) -> Report {
    let now = stamp();
    let result = if trusted(plugin) {
        Some(witness(plugin))
    } else {
        None
    };
    Report { plugin:plugin.uuid.clone(), version:plugin.version.clone(), actions:plugin.actions.iter().map(|a| {
        let (verdict, reason) = if a.uuid == ACTION {
            match &result { Some(Ok(())) => (Verdict::Passed, "Observé sur WebSocket : deux instances indépendantes, pas modifié pris en compte, titre et réglages rechargés après redémarrage du plugin".into()),
                Some(Err(e)) => (Verdict::Failed,e.clone()),
                None => (Verdict::Unverified,"Aucun résultat attendu connu pour ce plugin : non exécuté par le test".into()) }
        } else { (Verdict::Unverified,"Aucun résultat attendu déclaré pour cette action".into()) };
        ActionReport { action:a.uuid.clone(), verdict, reason, mode:"simulé (appareil virtuel, aucun USB)".into(), tested_at_unix:now }
    }).collect() }
}
fn binding(settings: Value) -> PluginBinding {
    PluginBinding {
        plugin: WITNESS.into(),
        action: ACTION.into(),
        settings,
        state: 0,
    }
}
fn configs() -> Config {
    let mut c = Config::default();
    c.keys_mut()[0].plugin = Some(binding(json!({"step":1,"count":0})));
    c.keys_mut()[1].plugin = Some(binding(json!({"step":1,"count":0})));
    c
}
// Reply::Message is the exact output of the production host. The UI also answers getSettings
// and persists setSettings; this harness keeps its own isolated virtual config instead.
fn observe(
    manager: &Manager,
    contexts: &HashMap<String, (usize, Value)>,
    target: &str,
    title: &str,
    timeout: Duration,
) -> Result<Value, String> {
    let until = Instant::now() + timeout;
    let mut seen_title = false;
    let mut updated = None;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(format!(
                "Délai dépassé : titre {title} et setSettings non observés"
            ));
        }
        match manager.rx.recv_timeout(left) {
            Ok(Reply::Error(_, reason)) => return Err(format!("Plugin en erreur : {reason}")),
            Ok(Reply::Message(_, msg)) => {
                let name = msg["event"].as_str().unwrap_or("");
                if name == "registerPlugin" {
                    continue;
                }
                let ctx = msg["context"].as_str().unwrap_or("");
                if name == "getSettings" {
                    let (_, settings) = contexts
                        .get(ctx)
                        .ok_or_else(|| format!("Contexte étranger : {ctx}"))?;
                    manager.send(WITNESS,json!({"event":"didReceiveSettings","action":ACTION,"context":ctx,"device":"deckforge-virtual","payload":{"settings":settings}}));
                    continue;
                }
                if !contexts.contains_key(ctx) {
                    return Err(format!("Contexte étranger : {ctx}"));
                }
                if ctx != target {
                    if matches!(name, "setTitle" | "setSettings") {
                        return Err(format!("Instance {ctx} modifiée par l'appui de {target}"));
                    }
                    continue;
                }
                match name {
                    "setTitle" => {
                        if msg["payload"]["title"] != title {
                            return Err(format!("Titre inattendu : {}", msg["payload"]["title"]));
                        }
                        seen_title = true;
                    }
                    "setSettings" => {
                        updated = msg["payload"]["settings"]
                            .as_object()
                            .map(|_| msg["payload"]["settings"].clone());
                        if updated.is_none() {
                            return Err("setSettings malformé".into());
                        }
                    }
                    "logMessage" => {}
                    other => return Err(format!("Message non pris en charge : {other}")),
                }
                if seen_title {
                    if let Some(s) = updated {
                        return Ok(s);
                    }
                }
            }
            Err(_) => {
                return Err(format!(
                    "Délai dépassé : titre {title} et setSettings non observés"
                ))
            }
        }
    }
}
fn press(
    manager: &Manager,
    contexts: &HashMap<String, (usize, Value)>,
    index: usize,
    title: &str,
) -> Result<Value, String> {
    let context = contexts
        .iter()
        .find(|(_, v)| v.0 == index)
        .map(|(k, _)| k.as_str())
        .ok_or("Contexte absent")?;
    manager.key(index, true);
    manager.key(index, false); // Same calls as physical Key / KeyRelease.
    observe(manager, contexts, context, title, LIMIT)
}
fn witness(plugin: &Plugin) -> Result<(), String> {
    let mut config = configs();
    let mut manager = Manager::new(vec![plugin.clone()]);
    manager.sync(&config);
    let mut contexts = manager
        .visible()
        .iter()
        .map(|(id, v)| (id.clone(), (v.index, v.settings.clone())))
        .collect::<HashMap<_, _>>();
    let first = press(&manager, &contexts, 0, "1")?;
    if first["count"] != 1 {
        return Err("Compteur non incrémenté après appui".into());
    }
    config.keys_mut()[0].plugin.as_mut().unwrap().settings = first;
    config.keys_mut()[0].plugin.as_mut().unwrap().settings["step"] = json!(3);
    manager.sync(&config);
    contexts = manager
        .visible()
        .iter()
        .map(|(id, v)| (id.clone(), (v.index, v.settings.clone())))
        .collect();
    let second = press(&manager, &contexts, 0, "4")?;
    if second["count"] != 4 {
        return Err("Changement de pas non appliqué".into());
    }
    config.keys_mut()[0].plugin.as_mut().unwrap().settings = second;
    let independent = press(&manager, &contexts, 1, "1")?;
    if independent["count"] != 1 {
        return Err("Instances non isolees".into());
    }
    config.keys_mut()[1].plugin.as_mut().unwrap().settings = independent;
    drop(manager); // A new Manager starts a new Node process; no user configuration is written.
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = dir.path().join("virtual-config.json");
    crate::store::save(&path, &config).map_err(|e| format!("Sauvegarde virtuelle : {e}"))?;
    let config = crate::store::load(&path).map_err(|e| format!("Relecture virtuelle : {e}"))?;
    let mut restarted = Manager::new(vec![plugin.clone()]);
    restarted.sync(&config);
    let contexts = restarted
        .visible()
        .iter()
        .map(|(id, v)| (id.clone(), (v.index, v.settings.clone())))
        .collect();
    let after = press(&restarted, &contexts, 0, "7")?;
    if after["count"] != 7 {
        return Err("Réglages non repris après redémarrage".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Plugin {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/counter.sdPlugin");
        crate::plugins::parse(
            &folder,
            &std::fs::read(folder.join("manifest.json")).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn real_node_virtual_device_counter() {
        let p = fixture();
        let report = simulate(&p);
        assert!(
            matches!(report.actions[0].verdict, Verdict::Passed),
            "{:?}",
            report.actions
        );
    }
    #[test]
    fn unknown_oracle_is_unverified_and_never_launched() {
        let mut p = fixture();
        p.uuid = "third.party".into();
        assert!(matches!(
            simulate(&p).actions[0].verdict,
            Verdict::Unverified
        ));
    }
    #[test]
    fn timeout_foreign_and_crash_are_not_successes() {
        let p = fixture();
        for (mode, expected) in [
            ("timeout", "Délai dépassé"),
            ("foreign", "Contexte étranger"),
            ("crash", "Plugin en erreur"),
            ("malformed", "Message JSON malformé"),
        ] {
            let mut config = configs();
            config.keys_mut()[0].plugin.as_mut().unwrap().settings = json!({"error":mode});
            let mut manager = Manager::new(vec![p.clone()]);
            manager.sync(&config);
            let contexts = manager
                .visible()
                .iter()
                .map(|(id, v)| (id.clone(), (v.index, v.settings.clone())))
                .collect();
            let err = press(&manager, &contexts, 0, "1").unwrap_err();
            assert!(err.contains(expected), "{mode}: {err}");
        }
    }
}
