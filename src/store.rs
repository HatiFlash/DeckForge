use crate::model::Config;
use anyhow::{Context, Result};
use std::{fs, io::Write, path::Path};

pub fn load(path: &Path) -> Result<Config> {
    let data = fs::read(path).context("Lecture de la configuration impossible")?;
    anyhow::ensure!(data.len() <= 8_000_000, "Configuration trop volumineuse");
    let mut c: Config = serde_json::from_slice(&data).context("Configuration JSON invalide")?;
    c.validate_structure()?;
    if c.schema == 1 {
        c.schema = 2;
    }
    Ok(c)
}
pub fn save(path: &Path, c: &Config) -> Result<()> {
    c.validate_structure()?;
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
    fn old_keys_keep_their_background_and_no_fill_preserves_the_saved_color() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut legacy = serde_json::to_value(Config::default()).unwrap();
        for key in legacy["profiles"][0]["keys"].as_array_mut().unwrap() {
            key.as_object_mut().unwrap().remove("background");
        }
        std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let mut config = load(&path).unwrap();
        assert!(config.keys().iter().all(|k| k.background));
        let original = config.keys()[1].color;
        config.keys_mut()[1].background = false;
        save(&path, &config).unwrap();
        let saved = load(&path).unwrap();
        assert!(!saved.keys()[1].background);
        assert_eq!(saved.keys()[1].color, original);
        assert!(saved.keys()[0].background);
    }
    #[test]
    fn old_configuration_migrates_without_losing_keys_or_images() {
        let mut old = serde_json::to_value(Config::default()).unwrap();
        old["schema"] = 1.into();
        old.as_object_mut().unwrap().remove("page");
        old.as_object_mut().unwrap().remove("trail");
        old["profiles"][0].as_object_mut().unwrap().remove("pages");
        old["profiles"][0]["keys"][0]["image"] = "C:\\icons\\my-image.png".into();
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("config.json");
        let original = serde_json::to_vec(&old).unwrap();
        fs::write(&path, &original).unwrap();
        let c = load(&path).unwrap();
        assert_eq!(c.schema, 2);
        assert_eq!(c.keys()[0].image, "C:\\icons\\my-image.png");
        assert_eq!(c.keys()[1].label, "Lecture");
        assert_eq!(fs::read(&path).unwrap(), original);
        save(&path, &c).unwrap();
        assert_eq!(fs::read(path.with_extension("json.bak")).unwrap(), original);
    }
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
