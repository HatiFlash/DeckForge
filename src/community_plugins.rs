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
    #[serde(default)] pub uuid: Option<String>,
    #[serde(default)] pub category: Option<String>,
    #[serde(default)] pub summary: Option<String>,
    #[serde(default)] pub summary_fr: Option<String>,
    #[serde(default)] pub icon: Option<String>,
    #[serde(default)] pub screenshots: Vec<String>,
    #[serde(default)] pub actions: Vec<Action>,
    #[serde(default)] pub tags: Vec<String>,
    #[serde(default)] pub requires: Option<String>,
    #[serde(skip)] pub compatible: bool,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Action { pub name: String, #[serde(default)] pub name_fr: Option<String> }
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
        if let Some(uuid) = &entry.uuid { anyhow::ensure!(!uuid.is_empty() && uuid.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c,b'.'|b'-')), "UUID invalide"); }
        if let Some(category) = &entry.category { anyhow::ensure!(["system","audio","streaming","productivity","smart-home","communication","gaming","utilities"].contains(&category.as_str()), "Catégorie inconnue"); }
        for text in [&entry.summary, &entry.summary_fr] { if let Some(text) = text { anyhow::ensure!(text.chars().count() <= 140, "Résumé trop long"); } }
        if let Some(requires) = &entry.requires { anyhow::ensure!(requires.chars().count() <= 200, "Prérequis trop long"); }
        if let Some(icon) = &entry.icon { https_url(icon)?; }
        anyhow::ensure!(entry.screenshots.len() <= 6 && entry.tags.len() <= 8 && entry.actions.len() <= 100, "Fiche communautaire trop longue");
        for screenshot in &entry.screenshots { https_url(screenshot)?; }
        anyhow::ensure!(entry.tags.iter().all(|tag| !tag.trim().is_empty() && tag.chars().count() <= 40), "Tag invalide");
        anyhow::ensure!(entry.actions.iter().all(|action| !action.name.trim().is_empty() && action.name.len() <= 120 && action.name_fr.as_ref().is_none_or(|s| s.len() <= 120)), "Action invalide");
    }
    Ok(entries)
}
pub fn fetch() -> Result<Vec<Entry>> { parse(&crate::https_get::get(REGISTRY_URL, REGISTRY_LIMIT)?) }

/// Network and image decoding are performed by callers on a worker thread, never on the UI thread.
pub fn image(url: &str, icon: bool) -> Result<eframe::egui::ColorImage> {
    https_url(url)?;
    let limit = if icon { 256 * 1024 } else { 2 * 1024 * 1024 };
    let bytes = crate::https_get::get(url, limit)?;
    let format = image::guess_format(&bytes)?;
    anyhow::ensure!(matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) && (!icon || format == image::ImageFormat::Png), "Format d'image invalide");
    let reader = image::ImageReader::with_format(std::io::Cursor::new(&bytes), format);
    let (w,h) = reader.into_dimensions()?;
    anyhow::ensure!(w > 0 && h > 0 && w <= 4096 && h <= 4096 && (!icon || w == h), "Dimensions d'image invalides");
    let decoded = image::load_from_memory_with_format(&bytes, format)?.to_rgba8();
    Ok(eframe::egui::ColorImage::from_rgba_unmultiplied([w as usize,h as usize], decoded.as_raw()))
}

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
    /// Contrôle réseau réel du catalogue publié : `cargo test live_registry -- --ignored`.
    #[test] #[ignore] fn live_registry_images_and_installs() {
        let entries = fetch().expect("catalogue publié lisible");
        assert!(!entries.is_empty());
        for entry in &entries {
            assert!(entry.compatible, "{} incompatible", entry.id);
            if let Some(icon) = &entry.icon { image(icon, true).unwrap_or_else(|e| panic!("icône {}: {e}", entry.id)); }
            for shot in &entry.screenshots { image(shot, false).unwrap_or_else(|e| panic!("capture {}: {e}", entry.id)); }
            let dest = tempfile::tempdir().unwrap();
            let installed = install(entry, dest.path()).unwrap_or_else(|e| panic!("installation {}: {e:#}", entry.id));
            println!("LIVE {} {} installed={installed}", entry.id, entry.version);
        }
    }
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
