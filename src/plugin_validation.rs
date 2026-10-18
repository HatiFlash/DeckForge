//! Virtual-device validation: same Manager::key routing as Event::Key/KeyRelease, never USB.
use crate::plugins::{Plugin, Runtime};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const WITNESS: &str = "org.deckforge.counter";
const ACTION: &str = "org.deckforge.counter.increment";

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
            match &result { Some(Ok(())) => (Verdict::Passed, "Observé via App et WebSocket Node : keyDown/keyUp, deux instances isolées, titres 1/4/1/7, getSettings/didReceiveSettings/setSettings sauvegardés et rechargés, getGlobalSettings/setGlobalSettings persistés et relus, image SVG, état 1, showOk/showAlert et willDisappear sur changement de page".into()),
                Some(Err(e)) => (Verdict::Failed,e.clone()),
                None => (Verdict::Unverified,"Aucun résultat attendu connu pour ce plugin : non exécuté par le test".into()) }
        } else { (Verdict::Unverified,"Aucun résultat attendu déclaré pour cette action".into()) };
        ActionReport { action:a.uuid.clone(), verdict, reason, mode:"simulé (appareil virtuel, aucun USB)".into(), tested_at_unix:now }
    }).collect() }
}
fn witness(plugin: &Plugin) -> Result<(), String> {
    crate::ui::validate_witness(plugin, None)
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
            let err = crate::ui::validate_witness(&p, Some(mode)).unwrap_err();
            assert!(err.contains(expected), "{mode}: {err}");
        }
    }
}
