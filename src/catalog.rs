use crate::model::{validate_key, Config, Key, Step};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};

#[derive(Clone)]
pub struct Preset {
    pub category: &'static str,
    pub description: &'static str,
    pub key: Key,
}

pub fn presets() -> Vec<Preset> {
    let mut items = Vec::new();
    let mut add = |category, label: &str, description, color, steps| {
        items.push(Preset {
            category,
            description,
            key: Key {
                label: label.into(),
                color,
                steps,
                ..Key::default()
            },
        });
    };
    for (label, command) in [
        ("Lecture / pause", "play_pause"),
        ("Piste suivante", "next"),
        ("Piste précédente", "previous"),
        ("Arrêter la lecture", "stop"),
        ("Volume +", "volume_up"),
        ("Volume −", "volume_down"),
        ("Muet", "mute"),
    ] {
        add(
            "Média",
            label,
            "Commande multimédia Windows, envoyée à l’application qui gère la lecture.",
            [76, 166, 137],
            vec![Step::Media {
                command: command.into(),
            }],
        );
    }
    for (label, keys, description) in [
        (
            "Copier",
            "CTRL+C",
            "Copier la sélection dans la fenêtre active.",
        ),
        (
            "Coller",
            "CTRL+V",
            "Coller le presse-papiers dans la fenêtre active.",
        ),
        (
            "Couper",
            "CTRL+X",
            "Couper la sélection dans la fenêtre active.",
        ),
        (
            "Annuler",
            "CTRL+Z",
            "Annuler la dernière modification dans la fenêtre active.",
        ),
        (
            "Rétablir",
            "CTRL+Y",
            "Rétablir dans les applications utilisant Ctrl+Y.",
        ),
        (
            "Tout sélectionner",
            "CTRL+A",
            "Sélectionner tout dans la fenêtre active.",
        ),
        ("Enregistrer", "CTRL+S", "Enregistrer le document actif."),
        (
            "Rechercher",
            "CTRL+F",
            "Ouvrir la recherche dans l’application active.",
        ),
        (
            "Imprimer",
            "CTRL+P",
            "Ouvrir le dialogue d’impression ; aucune impression automatique.",
        ),
    ] {
        add(
            "Édition",
            label,
            description,
            [108, 135, 236],
            vec![Step::Hotkey { keys: keys.into() }],
        );
    }
    for (label, keys, description) in [
        (
            "Explorateur",
            "WIN+E",
            "Ouvrir l’Explorateur de fichiers Windows.",
        ),
        ("Bureau", "WIN+D", "Afficher ou masquer le bureau Windows."),
        (
            "Changer de fenêtre",
            "ALT+TAB",
            "Passer à la fenêtre précédente.",
        ),
        (
            "Vue des tâches",
            "WIN+TAB",
            "Afficher les fenêtres et les bureaux virtuels.",
        ),
        (
            "Capture d’écran",
            "WIN+SHIFT+S",
            "Ouvrir la sélection de capture Windows.",
        ),
        (
            "Presse-papiers",
            "WIN+V",
            "Ouvrir l’historique du presse-papiers, s’il est activé.",
        ),
        ("Paramètres", "WIN+I", "Ouvrir les paramètres Windows."),
        (
            "Gestionnaire des tâches",
            "CTRL+SHIFT+ESC",
            "Ouvrir le Gestionnaire des tâches.",
        ),
    ] {
        add(
            "Windows",
            label,
            description,
            [73, 160, 217],
            vec![Step::Hotkey { keys: keys.into() }],
        );
    }
    for (label, keys, description) in [
        (
            "Nouvel onglet",
            "CTRL+T",
            "Dans ton navigateur actif, ouvrir un onglet.",
        ),
        (
            "Rouvrir un onglet",
            "CTRL+SHIFT+T",
            "Rouvrir le dernier onglet fermé du navigateur actif.",
        ),
        (
            "Onglet suivant",
            "CTRL+TAB",
            "Passer à l’onglet suivant de la fenêtre active.",
        ),
        (
            "Onglet précédent",
            "CTRL+SHIFT+TAB",
            "Passer à l’onglet précédent de la fenêtre active.",
        ),
        (
            "Barre d’adresse",
            "CTRL+L",
            "Sélectionner l’adresse dans le navigateur actif.",
        ),
        (
            "Actualiser",
            "CTRL+R",
            "Actualiser la page du navigateur actif.",
        ),
    ] {
        add(
            "Navigateur",
            label,
            description,
            [176, 128, 221],
            vec![Step::Hotkey { keys: keys.into() }],
        );
    }
    for (label, url) in [
        ("YouTube", "https://www.youtube.com"),
        ("GitHub", "https://github.com"),
        ("ChatGPT", "https://chatgpt.com"),
        ("Claude", "https://claude.ai"),
    ] {
        add(
            "Sites",
            label,
            "Ouvrir ce site dans le navigateur par défaut. Adresse modifiable après ajout.",
            [218, 149, 74],
            vec![Step::Url { url: url.into() }],
        );
    }
    add("Workflows", "Copier → autre fenêtre → coller", "Copier la sélection, passer à la fenêtre précédente, puis coller. Vérifie les deux fenêtres avant de lancer.", [203, 120, 143], vec![Step::Hotkey { keys: "CTRL+C".into() }, Step::Delay { ms: 200 }, Step::Hotkey { keys: "ALT+TAB".into() }, Step::Delay { ms: 400 }, Step::Hotkey { keys: "CTRL+V".into() }]);
    add(
        "Workflows",
        "Enregistrer puis rechercher",
        "Dans l’application active : enregistrer, attendre brièvement, puis ouvrir sa recherche.",
        [203, 120, 143],
        vec![
            Step::Hotkey {
                keys: "CTRL+S".into(),
            },
            Step::Delay { ms: 250 },
            Step::Hotkey {
                keys: "CTRL+F".into(),
            },
        ],
    );
    add("Workflows", "Signature de message", "Insérer un texte personnalisable dans la fenêtre active. Modifie le texte dans l’inspecteur.", [203, 120, 143], vec![Step::Text { text: "Bonne journée !".into() }]);
    items
}

pub fn action(kind: usize, profile: &str) -> Key {
    let step = match kind {
        0 => Step::Hotkey {
            keys: "CTRL+C".into(),
        },
        1 => Step::Text {
            text: "Mon texte".into(),
        },
        2 => Step::Launch {
            program: "explorer.exe".into(),
            args: vec![],
        },
        3 => Step::Url {
            url: "https://example.com".into(),
        },
        4 => Step::Media {
            command: "play_pause".into(),
        },
        5 => Step::Delay { ms: 500 },
        6 => Step::WaitWindow {
            title: "Mon application".into(),
            timeout_ms: 10_000,
            focus: true,
        },
        8 => Step::PageNext,
        9 => Step::PagePrevious,
        10 => Step::Back,
        11 => Step::Page { id: String::new() },
        _ => Step::Profile {
            name: profile.into(),
        },
    };
    Key {
        label: step.name().into(),
        color: [108, 135, 236],
        steps: vec![step],
        ..Key::default()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedBind {
    pub name: String,
    pub key: Key,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Library {
    pub schema: u32,
    pub binds: Vec<SavedBind>,
}
impl Default for Library {
    fn default() -> Self {
        Self {
            schema: 1,
            binds: vec![],
        }
    }
}
impl Library {
    pub fn validate(&self, config: &Config) -> Result<()> {
        ensure!(
            self.schema == 1 && self.binds.len() <= 128,
            "Bibliothèque invalide (128 binds maximum)"
        );
        let mut names = std::collections::HashSet::new();
        for bind in &self.binds {
            ensure!(
                !bind.name.trim().is_empty()
                    && bind.name.chars().count() <= 80
                    && names.insert(bind.name.to_lowercase()),
                "Noms de binds invalides ou en double"
            );
            ensure!(
                !bind.key.steps.is_empty(),
                "Un bind doit contenir au moins une action"
            );
            validate_key(&bind.key, config)?;
        }
        Ok(())
    }
    pub fn insert(&mut self, name: &str, key: Key, config: &Config) -> Result<()> {
        let name = name.trim().to_string();
        ensure!(
            !self
                .binds
                .iter()
                .any(|b| b.name.eq_ignore_ascii_case(&name)),
            "Ce nom existe déjà dans Mes binds"
        );
        let mut next = self.clone();
        next.binds.push(SavedBind { name, key });
        next.validate(config)?;
        *self = next;
        Ok(())
    }
    pub fn merge(&mut self, other: Library, config: &Config) -> Result<usize> {
        other.validate(config)?;
        let count = other.binds.len();
        let mut next = self.clone();
        for bind in other.binds {
            next.insert(&bind.name, bind.key, config)?;
        }
        *self = next;
        Ok(count)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub dark: bool,
    pub library: Library,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            dark: true,
            library: Library::default(),
        }
    }
}
impl Preferences {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = std::fs::read(path)?;
        ensure!(data.len() <= 8_000_000, "Préférences trop volumineuses");
        let p: Self = serde_json::from_slice(&data)?;
        ensure!(
            p.library.schema == 1 && p.library.binds.len() <= 128,
            "Bibliothèque invalide"
        );
        Ok(p)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        let mut tmp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        let bytes = serde_json::to_vec_pretty(self)?;
        ensure!(bytes.len() <= 8_000_000, "Préférences trop volumineuses");
        tmp.write_all(&bytes)?;
        tmp.as_file().sync_all()?;
        if path.exists() {
            std::fs::copy(path, path.with_extension("json.bak"))?;
        }
        tmp.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_preset_and_action_is_executable_with_valid_configuration() {
        let c = Config::default();
        assert_eq!(presets().len(), 37);
        for p in presets() {
            validate_key(&p.key, &c).unwrap();
            assert!(!p.key.steps.is_empty());
        }
        for i in 0..12 {
            validate_key(&action(i, "Bureau"), &c).unwrap();
        }
    }
    #[test]
    fn import_is_atomic_and_rejects_duplicate_names_and_invalid_macros() {
        let c = Config::default();
        let mut l = Library::default();
        l.insert("Lecture", action(4, "Bureau"), &c).unwrap();
        let mut incoming = Library::default();
        incoming.insert("Nouveau", action(0, "Bureau"), &c).unwrap();
        incoming.insert("Lecture", action(1, "Bureau"), &c).unwrap();
        assert!(l.merge(incoming, &c).is_err());
        assert_eq!(l.binds.len(), 1);
        let mut bad = action(0, "Bureau");
        bad.steps = vec![Step::Hotkey {
            keys: "BADKEY".into(),
        }];
        assert!(l.insert("Invalide", bad, &c).is_err());
        assert_eq!(l.binds.len(), 1);
    }
    #[test]
    fn theme_and_personal_binds_survive_restart_with_backup() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("preferences.json");
        let mut p = Preferences::default();
        p.library
            .insert("Mon bind", action(4, "Bureau"), &Config::default())
            .unwrap();
        p.save(&path).unwrap();
        p.dark = false;
        p.save(&path).unwrap();
        let loaded = Preferences::load(&path).unwrap();
        assert!(!loaded.dark);
        assert_eq!(loaded.library.binds[0].name, "Mon bind");
        assert!(
            Preferences::load(&path.with_extension("json.bak"))
                .unwrap()
                .dark
        );
    }
}
