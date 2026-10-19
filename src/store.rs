use crate::model::Config;
use anyhow::{Context, Result};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_BACKUPS: usize = 5;

fn load_one(path: &Path) -> Result<Config> {
    let data = fs::read(path).context("Lecture de la configuration impossible")?;
    anyhow::ensure!(data.len() <= 8_000_000, "Configuration trop volumineuse");
    let mut c: Config = serde_json::from_slice(&data).context("Configuration JSON invalide")?;
    c.validate_structure()?;
    if c.schema == 1 {
        c.schema = 2;
    }
    Ok(c)
}

pub fn list_backups(path: &Path) -> Vec<PathBuf> {
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    let backup_dir = parent.join("backups");
    let Ok(entries) = fs::read_dir(backup_dir) else {
        return Vec::new();
    };
    let mut backups = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("config-") && name.ends_with(".bak") {
                    backups.push(p);
                }
            }
        }
    }
    backups.sort();
    backups
}

#[allow(dead_code)]
pub fn restore_backup(target_path: &Path, backup_path: &Path) -> Result<Config> {
    let config = load_one(backup_path)?;
    save(target_path, &config)?;
    Ok(config)
}

pub fn load(path: &Path) -> Result<Config> {
    match load_one(path) {
        Ok(config) => Ok(config),
        Err(error) if path.extension().is_some_and(|e| e == "json") => {
            let backup = path.with_extension("json.bak");
            let backup_res = load_one(&backup);
            if let Ok(config) = backup_res {
                let message = format!(
                    "DeckForge : configuration illisible ({error}); sauvegarde {} restaurée en mémoire (fichier original conservé)\n",
                    backup.display()
                );
                eprintln!("{message}");
                let _ = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path.with_file_name("deckforge-error.log"))
                    .and_then(|mut file| file.write_all(message.as_bytes()));
                return Ok(config);
            }
            for b in list_backups(path).into_iter().rev() {
                if let Ok(config) = load_one(&b) {
                    let message = format!(
                        "DeckForge : configuration illisible ({error}); point de restauration {} utilisé en mémoire\n",
                        b.display()
                    );
                    eprintln!("{message}");
                    return Ok(config);
                }
            }
            Err(error).with_context(|| {
                format!(
                    "Configuration invalide et aucune sauvegarde fonctionnelle trouvée pour {}",
                    path.display()
                )
            })
        }
        Err(error) => Err(error),
    }
}

fn rotate_backups(backup_dir: &Path, path: &Path, content: &[u8]) -> Result<()> {
    fs::create_dir_all(backup_dir)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let new_backup = backup_dir.join(format!("config-{now}.bak"));
    let mut tmp = tempfile::NamedTempFile::new_in(backup_dir)?;
    tmp.write_all(content)?;
    tmp.as_file().sync_all()?;
    tmp.persist(&new_backup).map_err(|e| e.error)?;

    let mut existing = list_backups(path);
    if existing.len() > MAX_BACKUPS {
        let to_remove = existing.len() - MAX_BACKUPS;
        for p in existing.drain(..to_remove) {
            let _ = fs::remove_file(p);
        }
    }
    Ok(())
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
        // Ne jamais remplacer une sauvegarde saine par un fichier source corrompu.
        if load_one(path).is_ok() {
            let existing_data = fs::read(path)?;
            let mut backup = tempfile::NamedTempFile::new_in(parent)?;
            backup.write_all(&existing_data)?;
            backup.as_file().sync_all()?;
            backup
                .persist(path.with_extension("json.bak"))
                .map_err(|e| e.error)?;
            let _ = rotate_backups(&parent.join("backups"), path, &existing_data);
        }
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
    fn corrupted_main_recovers_backup_and_never_overwrites_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut c = Config::default();
        save(&path, &c).unwrap();
        c.brightness = 42;
        save(&path, &c).unwrap();
        let backup = path.with_extension("json.bak");
        let original_backup = fs::read(&backup).unwrap();
        fs::write(&path, b"{invalid").unwrap();
        assert_eq!(load(&path).unwrap().brightness, 70);
        save(&path, &c).unwrap();
        assert_eq!(fs::read(&backup).unwrap(), original_backup);
        assert_eq!(load(&path).unwrap().brightness, 42);
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

    #[test]
    fn rotates_and_restores_timestamped_backups() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut c = Config::default();
        save(&path, &c).unwrap();
        for b in 1..=7 {
            c.brightness = b * 10;
            save(&path, &c).unwrap();
        }
        let backups = list_backups(&path);
        assert!(backups.len() <= MAX_BACKUPS);
        assert!(!backups.is_empty());
        let restored = restore_backup(&path, &backups[0]).unwrap();
        assert_eq!(load(&path).unwrap().brightness, restored.brightness);
    }
}
