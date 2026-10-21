//! Registre communautaire : validation avant affichage et vérification cryptographique avant import.
#![cfg(windows)]
use std::{collections::HashSet, fs, path::Path};
use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const REGISTRY_URL: &str = "https://raw.githubusercontent.com/HatiFlash/novydeck-plugins/main/registry.json";
pub const CONTRIBUTE_URL: &str = "https://github.com/HatiFlash/novydeck-plugins/blob/main/CONTRIBUTING.md";
const ARCHIVE_LIMIT: usize = 50 * 1024 * 1024;
const REGISTRY_LIMIT: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    #[serde(default)] pub description_fr: Option<String>,
    pub homepage: String,
    pub download: String,
    pub sha256: String,
    pub license: String,
    #[serde(default)] pub tested_decks: Vec<String>,
    pub min_novydeck: String,
    #[serde(skip)] pub compatible: bool,
}
#[derive(Deserialize)] struct Registry { schema: u32, plugins: Vec<Entry> }

fn https_url(raw: &str) -> Result<url::Url> {
    let parsed = url::Url::parse(raw).context("URL de plugin invalide")?;
    anyhow::ensure!(parsed.scheme() == "https" && parsed.host_str().is_some() && parsed.username().is_empty() && parsed.password().is_none(), "Adresse HTTPS sans identifiants requise");
    Ok(parsed)
}
fn numeric_version(s: &str) -> Result<Vec<u64>> {
    anyhow::ensure!(!s.is_empty() && s.len() <= 64 && s.split('.').count() <= 8, "Version invalide");
    s.split('.').map(|part| {
        anyhow::ensure!(!part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()), "Version invalide");
        part.parse().context("Version invalide")
    }).collect()
}
fn compatible(required: &str, current: &str) -> Result<bool> {
    let mut required = numeric_version(required)?;
    let mut current = numeric_version(current)?;
    let count = required.len().max(current.len());
    required.resize(count, 0);
    current.resize(count, 0);
    Ok(current >= required)
}
pub fn parse(bytes: &[u8]) -> Result<Vec<Entry>> {
    anyhow::ensure!(bytes.len() <= REGISTRY_LIMIT, "Catalogue communautaire trop volumineux");
    let registry: Registry = serde_json::from_slice(bytes).context("Catalogue communautaire invalide")?;
    anyhow::ensure!(registry.schema == 1 && registry.plugins.len() <= 1000, "Format du catalogue communautaire non pris en charge");
    let mut seen = HashSet::new();
    let mut entries = registry.plugins;
    for entry in &mut entries {
        anyhow::ensure!(!entry.id.is_empty() && entry.id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-')) && seen.insert(entry.id.clone()), "Identifiant de plugin invalide ou en double");
        anyhow::ensure!(!entry.name.trim().is_empty() && !entry.author.trim().is_empty() && !entry.description.trim().is_empty() && !entry.license.trim().is_empty(), "Fiche communautaire incomplète");
        numeric_version(&entry.version)?;
        https_url(&entry.homepage)?;
        let download = https_url(&entry.download)?;
        anyhow::ensure!(download.path().ends_with(".streamDeckPlugin") || download.path().ends_with(".zip"), "Archive de plugin non prise en charge");
        anyhow::ensure!(entry.sha256.len() == 64 && entry.sha256.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)), "Empreinte SHA-256 invalide");
        entry.compatible = compatible(&entry.min_novydeck, env!("CARGO_PKG_VERSION"))?;
    }
    Ok(entries)
}
pub fn fetch() -> Result<Vec<Entry>> { parse(&crate::https_get::get(REGISTRY_URL, REGISTRY_LIMIT)?) }

/// Le fichier temporaire vérifié est passé à l'importeur local existant, puis effacé même en cas d'erreur.
pub fn install(entry: &Entry, dest: &Path) -> Result<bool> {
    anyhow::ensure!(entry.compatible, "Version de NovyDeck insuffisante pour ce plugin");
    let url = https_url(&entry.download)?;
    anyhow::ensure!(url.path().ends_with(".streamDeckPlugin") || url.path().ends_with(".zip"), "Archive de plugin non prise en charge");
    let bytes = crate::https_get::get(&entry.download, ARCHIVE_LIMIT)?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    anyhow::ensure!(actual == entry.sha256, "Empreinte SHA-256 différente : rien n'a été installé");
    let temp = tempfile::tempdir().context("Création du dossier temporaire impossible")?;
    let name = if url.path().ends_with(".zip") { "plugin.zip" } else { "plugin.streamDeckPlugin" };
    let archive = temp.path().join(name);
    fs::write(&archive, bytes).context("Écriture temporaire impossible")?;
    crate::plugin_import::import_archive(&archive, dest)
}

#[cfg(test)] mod tests {
    use super::*;
    fn entry() -> serde_json::Value {
        serde_json::json!({"id":"com.author.myplugin","name":"My Plugin","author":"Author","version":"1.2.0","description":"English", "description_fr":"Français", "homepage":"https://github.com/author/plugin", "download":"https://github.com/author/plugin/releases/download/v1/plugin.streamDeckPlugin", "sha256":"a".repeat(64),"license":"MIT","tested_decks":["FIFINE D6"],"min_novydeck":"0.8.12"})
    }
    fn registry(value: serde_json::Value) -> Vec<u8> { serde_json::to_vec(&serde_json::json!({"schema":1,"plugins":[value]})).unwrap() }
    #[test] fn registry_valid_and_version_compatibility() {
        let mut value = entry();
        value["min_novydeck"] = "0.8.11".into();
        assert!(parse(&registry(value.clone())).unwrap()[0].compatible);
        value["min_novydeck"] = "999.0.0".into();
        assert!(!parse(&registry(value)).unwrap()[0].compatible);
        assert!(parse(br#"{"schema":1,"plugins":[]}"#).unwrap().is_empty());
    }
    #[test] fn registry_rejects_invalid_id_duplicate_http_and_sha() {
        let mut value = entry(); value["id"] = "BAD/id".into(); assert!(parse(&registry(value)).is_err());
        let value = entry();
        let duplicate = serde_json::to_vec(&serde_json::json!({"schema":1,"plugins":[value.clone(),value.clone()]})).unwrap();
        assert!(parse(&duplicate).is_err());
        let mut value = entry(); value["download"] = "http://example.org/x.zip".into(); assert!(parse(&registry(value)).is_err());
        let mut value = entry(); value["homepage"] = "http://example.org/".into(); assert!(parse(&registry(value)).is_err());
        let mut value = entry(); value["sha256"] = "A".repeat(64).into(); assert!(parse(&registry(value)).is_err());
    }
    #[test] fn rejects_oversized_registry_and_unknown_schema() {
        assert!(parse(&vec![b' '; REGISTRY_LIMIT + 1]).is_err());
        assert!(parse(br#"{"schema":2,"plugins":[]}"#).is_err());
    }
}
