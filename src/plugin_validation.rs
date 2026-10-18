//! Oracles locaux pour vérifier une action par observation, jamais par simple lancement.
use crate::plugins::Plugin;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(test)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleFile {
    pub schema_version: u32,
    pub plugin: String,
    pub version: String,
    pub actions: Vec<ActionOracle>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionOracle {
    pub action: String,
    pub mode: Mode,
    pub real_effects: bool,
    pub expected_effect: String,
    pub timeout_ms: u64,
    #[serde(default)]
    pub requirements: Requirements,
    pub observation: Observation,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirements {
    #[serde(default)]
    pub software: Vec<String>,
    #[serde(default)]
    pub account: Option<String>,
    #[serde(default)]
    pub hardware: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Simulated,
    Real,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Observation {
    Title {
        equals: String,
    },
    State {
        equals: u32,
    },
    Image {
        starts_with: String,
    },
    File {
        directory: Option<String>,
        extension: String,
    },
    // Processus, fenêtres et registre demandent une sonde lecture seule dédiée : non vérifiés jusque-là.
    WindowsRegistry {
        key: String,
        value: String,
        equals: String,
    },
    Process {
        name: String,
    },
    Window {
        title: String,
    },
}
fn safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() < 160
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        && !s.contains("..")
}
fn safe_directory(s: &str) -> bool {
    let p = Path::new(s);
    p.is_absolute()
        && !s.contains("..")
        && !s.starts_with("\\")
        && p.components().all(|c| {
            matches!(
                c,
                std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
                    | std::path::Component::Normal(_)
            )
        })
}
impl OracleFile {
    pub fn parse(bytes: &[u8], plugin: &Plugin) -> Result<Self, String> {
        if bytes.len() > 65536 {
            return Err("Oracle trop volumineux".into());
        }
        let file: Self =
            serde_json::from_slice(bytes).map_err(|e| format!("Oracle invalide : {e}"))?;
        if file.schema_version != 1
            || file.plugin != plugin.uuid
            || file.version != plugin.version
            || !safe_id(&file.plugin)
            || !safe_id(&file.version)
            || file.actions.len() > 256
        {
            return Err("Identité, version ou schéma d'oracle incorrect".into());
        }
        let mut seen = std::collections::HashSet::new();
        for a in &file.actions {
            if !safe_id(&a.action)
                || !seen.insert(&a.action)
                || !plugin.actions.iter().any(|p| p.uuid == a.action)
                || a.timeout_ms < 100
                || a.timeout_ms > 10000
                || a.expected_effect.trim().is_empty()
                || a.expected_effect.len() > 500
                || a.real_effects != (a.mode == Mode::Real)
            {
                return Err(format!("Action oracle invalide : {}", a.action));
            }
            if a.requirements.software.iter().any(|s| !safe_directory(s))
                || a.requirements
                    .account
                    .as_ref()
                    .is_some_and(|s| s.trim().is_empty())
                || a.requirements
                    .hardware
                    .as_ref()
                    .is_some_and(|s| s.trim().is_empty())
            {
                return Err("Prérequis invalide".into());
            }
            match &a.observation {
                Observation::File {
                    directory,
                    extension,
                } if !matches!(
                    extension.as_str(),
                    "png" | "jpg" | "jpeg" | "bmp" | "webp" | "txt"
                ) || directory.as_ref().is_some_and(|s| !safe_directory(s)) =>
                {
                    return Err("Dossier ou motif de fichier invalide".into())
                }
                Observation::Title { equals } if equals.len() > 256 => {
                    return Err("Titre trop long".into())
                }
                Observation::Image { starts_with } if starts_with.len() > 256 => {
                    return Err("Image : motif trop long".into())
                }
                _ => {}
            }
        }
        Ok(file)
    }
}
pub fn data_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir())
        .join("DeckForge")
}
pub fn oracle_path(plugin: &Plugin) -> Option<PathBuf> {
    if safe_id(&plugin.uuid) && safe_id(&plugin.version) {
        Some(
            data_root()
                .join("plugin-oracles")
                .join(format!("{}-{}.json", plugin.uuid, plugin.version)),
        )
    } else {
        None
    }
}
pub fn report_path(plugin: &Plugin) -> Option<PathBuf> {
    if safe_id(&plugin.uuid) && safe_id(&plugin.version) {
        Some(
            data_root()
                .join("plugin-reports")
                .join(format!("{}-{}.json", plugin.uuid, plugin.version)),
        )
    } else {
        None
    }
}
pub fn load_oracle(plugin: &Plugin) -> Result<Option<OracleFile>, String> {
    let Some(path) = oracle_path(plugin) else {
        return Ok(None);
    };
    match std::fs::read(path) {
        Ok(bytes) => OracleFile::parse(&bytes, plugin).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
pub fn save_report(plugin: &Plugin, report: &Report) -> Result<(), String> {
    let p = report_path(plugin).ok_or("Identité de plugin invalide")?;
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(
        p,
        serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
#[cfg(test)]
pub fn simulate(plugin: &Plugin) -> Report {
    run(plugin, load_oracle(plugin).ok().flatten().as_ref(), false)
}
// Fail-closed : le WebSocket de l'hôte ne confine pas les API du navigateur.
// Un refus n'observe aucune tentative JavaScript ; il ne doit pas être un succès.
const HTML_VALIDATION_REFUSED: &str = "HTML refusé : CDP absent avant script ; popup/window.open, navigation/redirections et réseau (fetch, XHR, WebSocket) non bloqués ni observés ; aucun HTML exécuté";

pub fn run(plugin: &Plugin, oracle: Option<&OracleFile>, confirmed_real: bool) -> Report {
    let now = stamp();
    Report {
        plugin: plugin.uuid.clone(),
        version: plugin.version.clone(),
        actions: plugin
            .actions
            .iter()
            .map(|a| {
                let entry = oracle.and_then(|o| o.actions.iter().find(|x| x.action == a.uuid));
                let mode = entry
                    .map(|o| {
                        if o.mode == Mode::Real {
                            "réel"
                        } else {
                            "simulé"
                        }
                    })
                    .unwrap_or("simulé");
                // Sans CDP attaché avant tout script et contrôle des popups/requêtes,
                // aucune page HTML n'est chargée pendant une validation, même sans oracle.
                let (verdict, reason) =
                    if matches!(plugin.runtime, crate::plugins::Runtime::Html(_)) {
                        (Verdict::Unverified, HTML_VALIDATION_REFUSED.into())
                    } else {
                        match entry {
                            None => (
                                Verdict::Unverified,
                                "Aucun résultat attendu déclaré pour cette action".into(),
                            ),
                            Some(o) if o.real_effects && !confirmed_real => (
                                Verdict::Unverified,
                                "Effets réels : confirmation explicite requise ; non exécuté"
                                    .into(),
                            ),
                            Some(o) if o.mode == Mode::Simulated && confirmed_real => (
                                Verdict::Unverified,
                                "Action simulée : non exécutée pendant l'essai réel".into(),
                            ),
                            Some(o) => {
                                let missing = o
                                    .requirements
                                    .software
                                    .iter()
                                    .find(|s| !Path::new(s).exists())
                                    .map(|s| format!("Logiciel absent : {s}"));
                                let missing = missing
                                    .or_else(|| {
                                        o.requirements
                                            .account
                                            .as_ref()
                                            .map(|s| format!("Compte non vérifié : {s}"))
                                    })
                                    .or_else(|| {
                                        o.requirements
                                            .hardware
                                            .as_ref()
                                            .map(|s| format!("Matériel non vérifié : {s}"))
                                    });
                                if let Some(reason) = missing {
                                    (Verdict::Unverified, reason)
                                } else if matches!(
                                    &o.observation,
                                    Observation::File {
                                        directory: None,
                                        ..
                                    }
                                ) {
                                    (
                                        Verdict::Unverified,
                                        "Dossier de destination inconnu ; aucune exécution".into(),
                                    )
                                } else if matches!(
                                    &o.observation,
                                    Observation::WindowsRegistry { .. }
                                        | Observation::Process { .. }
                                        | Observation::Window { .. }
                                ) {
                                    (
                                        Verdict::Unverified,
                                        "Sonde Windows indisponible ; aucune exécution".into(),
                                    )
                                } else {
                                    match crate::ui::validate_oracle(plugin, &a.uuid, o) {
                                        Ok(s) => (Verdict::Passed, s),
                                        Err(e) => (Verdict::Failed, e),
                                    }
                                }
                            }
                        }
                    };
                ActionReport {
                    action: a.uuid.clone(),
                    verdict,
                    reason,
                    mode: mode.into(),
                    tested_at_unix: now,
                }
            })
            .collect(),
    }
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
        crate::ui::validate_witness(&p, None).unwrap();
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "requires pinned OpenAction counter v1.0.1 release archive; starts third-party native process"]
    fn imported_openaction_counter_changes_title_in_virtual_app() {
        use sha2::{Digest, Sha256};
        let archive = std::path::PathBuf::from(std::env::var_os("DECKFORGE_TEST_OACOUNTER_ZIP").expect("set DECKFORGE_TEST_OACOUNTER_ZIP"));
        let bytes = std::fs::read(&archive).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), "f04e0724188468e450633a49d15ee98ab0400faa2197332cfcf80278d0c54e47");
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("plugins");
        assert!(crate::plugin_import::import_archive(&archive, &root).unwrap());
        let plugin = crate::plugin_import::inventory(&root).into_iter().find(|p| p.uuid == "me.amankhanna.oacounter").expect("imported counter missing");
        let oracle = ActionOracle {
            action: "me.amankhanna.oacounter.persisted".into(),
            mode: Mode::Simulated,
            real_effects: false,
            expected_effect: "counter title is 1".into(),
            timeout_ms: 15000,
            requirements: Requirements::default(),
            observation: Observation::Title { equals: "1".into() },
        };
        let report = crate::ui::validate_oracle(&plugin, &oracle.action, &oracle).unwrap();
        assert!(report.contains("Observation confirmée après appui"));
    }
    #[test]
    fn html_oracle_never_starts_unisolated_browser() {
        let mut p = fixture();
        p.runtime = crate::plugins::Runtime::Html(p.folder.join("app.html"));
        let o = oracle(&p, ACTION, Observation::Title { equals: "1".into() });
        let result = run(&p, Some(&o), false);
        assert!(matches!(result.actions[0].verdict, Verdict::Unverified));
        assert!(result.actions[0].reason.contains("CDP absent avant script"));
    }
    #[test]
    fn html_fixture_refuses_with_and_without_oracle() {
        // Fixture entièrement maîtrisée : chacune de ces API exigerait un blocage
        // navigateur avant script ; le garde refuse au lieu de simuler leur interception.
        let dir = tempfile::tempdir().unwrap();
        let page = dir.path().join("index.html");
        std::fs::write(
            &page,
            r#"<script>
            window.open('https://example.invalid/popup');
            location.href='https://example.invalid/navigation';
            fetch('https://example.invalid/fetch');
            new XMLHttpRequest().open('GET','https://example.invalid/xhr');
            new WebSocket('wss://example.invalid/socket');
        </script>"#,
        )
        .unwrap();
        let mut p = fixture();
        p.uuid = "fixture.html.generic".into();
        p.runtime = crate::plugins::Runtime::Html(page);
        for oracle in [
            None,
            Some(oracle(
                &p,
                ACTION,
                Observation::Title { equals: "1".into() },
            )),
        ] {
            let report = run(&p, oracle.as_ref(), false);
            assert!(report
                .actions
                .iter()
                .all(|a| matches!(a.verdict, Verdict::Unverified)));
            assert!(report
                .actions
                .iter()
                .all(|a| a.reason.contains("CDP absent avant script")
                    && a.reason.contains("non bloqués ni observés")));
        }
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
    fn oracle(p: &Plugin, action: &str, observation: Observation) -> OracleFile {
        OracleFile {
            schema_version: 1,
            plugin: p.uuid.clone(),
            version: p.version.clone(),
            actions: vec![ActionOracle {
                action: action.into(),
                mode: Mode::Simulated,
                real_effects: false,
                expected_effect: "Résultat témoin".into(),
                timeout_ms: 2000,
                requirements: Requirements::default(),
                observation,
            }],
        }
    }
    #[test]
    fn format_strict_and_invalid_paths() {
        let p = fixture();
        let o = oracle(&p, ACTION, Observation::Title { equals: "1".into() });
        assert!(OracleFile::parse(&serde_json::to_vec(&o).unwrap(), &p).is_ok());
        let mut bad = serde_json::to_value(&o).unwrap();
        bad["actions"][0]["observation"] =
            serde_json::json!({"type":"file","directory":"../outside","extension":"png"});
        assert!(OracleFile::parse(&serde_json::to_vec(&bad).unwrap(), &p).is_err());
        bad["actions"][0]["observation"] =
            serde_json::json!({"type":"title","equals":"1","command":"powershell"});
        assert!(OracleFile::parse(&serde_json::to_vec(&bad).unwrap(), &p).is_err());
    }
    #[test]
    fn no_oracle_or_prerequisite_or_confirmation() {
        let p = fixture();
        assert!(matches!(
            run(&p, None, false).actions[1].verdict,
            Verdict::Unverified
        ));
        let mut o = oracle(&p, ACTION, Observation::Title { equals: "1".into() });
        o.actions[0]
            .requirements
            .software
            .push("C:\\DeckForge-absent-987\\none.exe".into());
        assert!(matches!(
            run(&p, Some(&o), false).actions[0].verdict,
            Verdict::Unverified
        ));
        o.actions[0].requirements.software.clear();
        o.actions[0].mode = Mode::Real;
        o.actions[0].real_effects = true;
        assert!(matches!(
            run(&p, Some(&o), false).actions[0].verdict,
            Verdict::Unverified
        ));
    }
    #[test]
    fn title_and_file_observed_and_missing_file_fails() {
        let p = fixture();
        let title = oracle(&p, ACTION, Observation::Title { equals: "1".into() });
        let result = run(&p, Some(&title), false);
        assert!(
            matches!(result.actions[0].verdict, Verdict::Passed),
            "{:?}",
            result.actions
        );
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().to_string_lossy().into_owned();
        let file = oracle(
            &p,
            "org.deckforge.counter.write_file",
            Observation::File {
                directory: Some(folder.clone()),
                extension: "txt".into(),
            },
        );
        let result = run(&p, Some(&file), false);
        assert!(
            matches!(result.actions[1].verdict, Verdict::Passed),
            "{:?}",
            result.actions
        );
        std::fs::remove_file(dir.path().join("deckforge-witness.txt")).unwrap();
        let absent = oracle(
            &p,
            "org.deckforge.counter.write_file",
            Observation::File {
                directory: Some(folder),
                extension: "png".into(),
            },
        );
        let result = run(&p, Some(&absent), false);
        assert!(
            matches!(result.actions[1].verdict, Verdict::Failed),
            "{:?}",
            result.actions
        );
    }
}
