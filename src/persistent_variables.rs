use anyhow::{Context, Result};
use std::{collections::BTreeMap, fs, io::Write, path::{Path, PathBuf}, sync::{Mutex, OnceLock}};

struct State { path: PathBuf, values: BTreeMap<String, String> }
static STATE: OnceLock<Mutex<Option<State>>> = OnceLock::new();
fn state() -> &'static Mutex<Option<State>> { STATE.get_or_init(|| Mutex::new(None)) }

fn valid(values: &BTreeMap<String, String>) -> Result<()> {
    anyhow::ensure!(values.len() <= 256, "Variables enregistrées : 256 noms maximum");
    for (name, value) in values {
        anyhow::ensure!(!name.is_empty() && name.len() <= 64 && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'), "Variables enregistrées : nom invalide");
        anyhow::ensure!(value.len() <= 4096, "Variables enregistrées : valeur de {name} supérieure à 4 Ko");
    }
    Ok(())
}
fn load(path: &Path) -> Result<BTreeMap<String, String>> {
    if !path.exists() { return Ok(BTreeMap::new()); }
    let bytes = fs::read(path).context("Lecture de variables.json impossible")?;
    anyhow::ensure!(bytes.len() <= 1_100_000, "variables.json trop volumineux");
    let values: BTreeMap<String, String> = serde_json::from_slice(&bytes).context("variables.json invalide")?;
    valid(&values)?;
    Ok(values)
}
fn save(path: &Path, values: &BTreeMap<String, String>) -> Result<()> {
    valid(values)?;
    let parent = path.parent().context("Dossier des variables invalide")?;
    fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(&serde_json::to_vec_pretty(values)?)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
pub fn configure(path: &Path) -> Result<()> {
    let mut guard = state().lock().unwrap_or_else(|e| e.into_inner());
    *guard = None;
    let values = load(path)?;
    *guard = Some(State { path: path.to_owned(), values });
    Ok(())
}
pub fn snapshot() -> BTreeMap<String, String> {
    state().lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|s| s.values.clone()).unwrap_or_default()
}
fn change(name: &str, value: Option<&str>) -> Result<()> {
    let mut guard = state().lock().unwrap_or_else(|e| e.into_inner());
    let s = guard.as_mut().context("Variables enregistrées non initialisées")?;
    let mut next = s.values.clone();
    match value { Some(value) => { next.insert(name.to_owned(), value.to_owned()); }, None => { next.remove(name); } }
    valid(&next)?;
    save(&s.path, &next).context("Enregistrement des variables impossible")?;
    s.values = next;
    Ok(())
}
pub fn set(name: &str, value: &str) -> Result<()> { change(name, Some(value)) }
pub fn remove(name: &str) -> Result<()> { change(name, None) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_roundtrip_and_limits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("variables.json");
        let mut values = BTreeMap::new();
        values.insert("mode".into(), "été".into());
        save(&path, &values).unwrap();
        assert_eq!(load(&path).unwrap(), values);
        values.insert("mode".into(), "été".repeat(2049));
        assert!(save(&path, &values).is_err());
        assert_eq!(load(&path).unwrap()["mode"], "été");
        values.insert("mode".into(), "ok".into());
        for i in 0..256 { values.insert(format!("v{i}"), "x".into()); }
        assert!(save(&path, &values).is_err());
        fs::write(&path, br#"{"nom interdit":"x"}"#).unwrap();
        assert!(load(&path).is_err());
    }
}
