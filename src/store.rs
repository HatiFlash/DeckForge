use crate::model::Config;
use anyhow::{Context, Result};
use std::{fs, io::Write, path::Path};

pub fn load(path: &Path) -> Result<Config> {
    let data = fs::read(path).context("Lecture de la configuration impossible")?;
    anyhow::ensure!(data.len() <= 8_000_000, "Configuration trop volumineuse");
    let c: Config = serde_json::from_slice(&data).context("Configuration JSON invalide")?;
    c.validate()?;
    Ok(c)
}
pub fn save(path: &Path, c: &Config) -> Result<()> {
    c.validate()?;
    let parent = path.parent().context("Dossier de configuration invalide")?;
    fs::create_dir_all(parent)?;
    let bytes = serde_json::to_vec_pretty(c)?;
    anyhow::ensure!(bytes.len() <= 8_000_000, "Configuration trop volumineuse");
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(&bytes)?;
    tmp.as_file().sync_all()?;
    if path.exists() {
        fs::copy(path, path.with_extension("json.bak"))?;
    }
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_replaces_and_preserves_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut c = Config::default();
        save(&path, &c).unwrap();
        c.brightness = 42;
        save(&path, &c).unwrap();
        assert_eq!(load(&path).unwrap().brightness, 42);
        assert_eq!(
            load(&path.with_extension("json.bak")).unwrap().brightness,
            70
        );
    }
    #[test]
    fn invalid_save_leaves_good_config_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut c = Config::default();
        save(&path, &c).unwrap();
        c.active = 999;
        assert!(save(&path, &c).is_err());
        assert_eq!(load(&path).unwrap().active, 0);
    }
}
