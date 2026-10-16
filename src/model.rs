use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    Delay {
        ms: u64,
    },
    Hotkey {
        keys: String,
    },
    Text {
        text: String,
    },
    Launch {
        program: String,
        args: Vec<String>,
    },
    Url {
        url: String,
    },
    WaitWindow {
        title: String,
        timeout_ms: u64,
        focus: bool,
    },
    Media {
        command: String,
    },
    Profile {
        name: String,
    },
    Page {
        id: String,
    },
    Folder {
        id: String,
    },
    Back,
    PageNext,
    PagePrevious,
}
impl Step {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Delay { .. } => "Pause",
            Self::Hotkey { .. } => "Raccourci",
            Self::Text { .. } => "Saisir du texte",
            Self::Launch { .. } => "Lancer une application",
            Self::Url { .. } => "Ouvrir un site",
            Self::WaitWindow { .. } => "Attendre une fenêtre",
            Self::Media { .. } => "Média / volume",
            Self::Profile { .. } => "Changer de profil",
            Self::Page { .. } => "Aller à une page",
            Self::Folder { .. } => "Ouvrir un dossier",
            Self::Back => "Retour au dossier précédent",
            Self::PageNext => "Page suivante",
            Self::PagePrevious => "Page précédente",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Key {
    pub label: String,
    pub color: [u8; 3],
    pub image: String,
    pub repeat: u32,
    pub steps: Vec<Step>,
}
impl Default for Key {
    fn default() -> Self {
        Self {
            label: String::new(),
            color: [35, 44, 60],
            image: String::new(),
            repeat: 1,
            steps: vec![],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub keys: Vec<Key>,
    #[serde(default)]
    pub pages: Vec<Page>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Page {
    pub id: String,
    pub name: String,
    pub folder: bool,
    pub keys: Vec<Key>,
}
impl Page {
    pub fn new(id: String, name: String, folder: bool) -> Self {
        let mut keys = vec![Key::default(); 15];
        if folder {
            keys[0] = Key {
                label: "Retour".into(),
                color: [137, 165, 218],
                steps: vec![Step::Back],
                ..Key::default()
            };
        }
        Self {
            id,
            name,
            folder,
            keys,
        }
    }
}
impl Profile {
    pub fn new(name: String) -> Self {
        Self {
            name,
            keys: vec![Key::default(); 15],
            pages: vec![],
        }
    }
    pub fn keys_at(&self, id: &str) -> Option<&Vec<Key>> {
        if id.is_empty() {
            Some(&self.keys)
        } else {
            self.pages.iter().find(|p| p.id == id).map(|p| &p.keys)
        }
    }
    pub fn keys_at_mut(&mut self, id: &str) -> Option<&mut Vec<Key>> {
        if id.is_empty() {
            Some(&mut self.keys)
        } else {
            self.pages
                .iter_mut()
                .find(|p| p.id == id)
                .map(|p| &mut p.keys)
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub schema: u32,
    pub brightness: u8,
    pub active: usize,
    pub profiles: Vec<Profile>,
    // Firmware variants disagree on whether key input needs the display row flip.
    pub flip_input_rows: bool,
    pub image_size: u32,
    #[serde(default)]
    pub page: String,
    #[serde(default)]
    pub trail: Vec<String>,
}
impl Default for Config {
    fn default() -> Self {
        let mut profile = Profile::new("Bureau".into());
        profile.keys[0] = Key {
            label: "Exemple macro".into(),
            color: [83, 66, 175],
            steps: vec![
                Step::Delay { ms: 1500 },
                Step::Hotkey {
                    keys: "CTRL+C".into(),
                },
                Step::Delay { ms: 250 },
            ],
            ..Key::default()
        };
        profile.keys[1] = Key {
            label: "Lecture".into(),
            color: [30, 105, 99],
            steps: vec![Step::Media {
                command: "play_pause".into(),
            }],
            ..Key::default()
        };
        profile.keys[2] = Key {
            label: "Volume +".into(),
            steps: vec![Step::Media {
                command: "volume_up".into(),
            }],
            ..Key::default()
        };
        profile.keys[3] = Key {
            label: "Volume -".into(),
            steps: vec![Step::Media {
                command: "volume_down".into(),
            }],
            ..Key::default()
        };
        profile.keys[4] = Key {
            label: "Muet".into(),
            steps: vec![Step::Media {
                command: "mute".into(),
            }],
            ..Key::default()
        };
        Self {
            schema: 2,
            brightness: 70,
            active: 0,
            profiles: vec![profile],
            flip_input_rows: false,
            image_size: 126,
            page: String::new(),
            trail: vec![],
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        if ![1, 2].contains(&self.schema) {
            bail!("Version de configuration non prise en charge");
        }
        if self.profiles.is_empty()
            || self.profiles.len() > 64
            || self.active >= self.profiles.len()
        {
            bail!("Profils invalides (1 à 64)");
        }
        if self.brightness > 100 || ![112, 126].contains(&self.image_size) {
            bail!("Réglages du D6 invalides");
        }
        let mut names = std::collections::HashSet::new();
        for p in &self.profiles {
            if p.name.trim().is_empty() || !names.insert(p.name.to_lowercase()) {
                bail!("Chaque profil doit avoir un nom unique");
            }
            if p.pages.len() > 128 {
                bail!("128 pages et dossiers maximum par profil");
            }
            let mut ids = std::collections::HashSet::new();
            for page in &p.pages {
                if page.id.is_empty()
                    || page.id.len() > 160
                    || page.name.trim().is_empty()
                    || !ids.insert(&page.id)
                {
                    bail!("Identifiant ou nom de page invalide");
                }
            }
            for keys in std::iter::once(&p.keys).chain(p.pages.iter().map(|page| &page.keys)) {
                if keys.len() != 15 {
                    bail!("Chaque page doit contenir 15 touches");
                }
                for key in keys {
                    validate_key_in_profile(key, self, p)?;
                }
            }
        }
        let p = &self.profiles[self.active];
        if p.keys_at(&self.page).is_none()
            || self.trail.len() > 32
            || self.trail.iter().any(|id| p.keys_at(id).is_none())
        {
            bail!("Navigation invalide");
        }
        Ok(())
    }
    pub fn keys(&self) -> &Vec<Key> {
        self.profiles[self.active]
            .keys_at(&self.page)
            .unwrap_or(&self.profiles[self.active].keys)
    }
    pub fn keys_mut(&mut self) -> &mut Vec<Key> {
        let page = self.page.clone();
        self.profiles[self.active]
            .keys_at_mut(&page)
            .expect("validated navigation")
    }
    pub fn reset_navigation(&mut self) {
        self.page.clear();
        self.trail.clear();
    }
    pub fn navigate(&mut self, step: &Step) -> Result<()> {
        let p = &self.profiles[self.active];
        match step {
            Step::Folder { id } => {
                anyhow::ensure!(
                    p.pages.iter().any(|p| p.id == *id && p.folder),
                    "Dossier introuvable"
                );
                anyhow::ensure!(self.trail.len() < 32, "32 niveaux de dossiers maximum");
                self.trail.push(self.page.clone());
                self.page = id.clone();
            }
            Step::Page { id } => {
                anyhow::ensure!(
                    id.is_empty() || p.pages.iter().any(|p| p.id == *id && !p.folder),
                    "Page introuvable"
                );
                self.page = id.clone();
                self.trail.clear();
            }
            Step::Back => {
                self.page = self.trail.pop().unwrap_or_default();
            }
            Step::PageNext | Step::PagePrevious => {
                let roots = std::iter::once(String::new())
                    .chain(p.pages.iter().filter(|p| !p.folder).map(|p| p.id.clone()))
                    .collect::<Vec<_>>();
                let index = roots.iter().position(|id| *id == self.page).unwrap_or(0);
                let next = if matches!(step, Step::PageNext) {
                    (index + 1) % roots.len()
                } else {
                    (index + roots.len() - 1) % roots.len()
                };
                self.page = roots[next].clone();
                self.trail.clear();
            }
            _ => bail!("Action de navigation requise"),
        }
        Ok(())
    }
}
pub fn validate_key_in_profile(key: &Key, c: &Config, p: &Profile) -> Result<()> {
    validate_key(key, c)?;
    for step in &key.steps {
        match step {
            Step::Folder { id } => anyhow::ensure!(
                p.pages.iter().any(|p| p.id == *id && p.folder),
                "Dossier introuvable dans ce profil"
            ),
            Step::Page { id } => anyhow::ensure!(
                id.is_empty() || p.pages.iter().any(|p| p.id == *id && !p.folder),
                "Page introuvable dans ce profil"
            ),
            _ => {}
        }
    }
    Ok(())
}
pub fn media_vk(command: &str) -> Option<u16> {
    Some(match command {
        "play_pause" => 0xB3,
        "next" => 0xB0,
        "previous" => 0xB1,
        "stop" => 0xB2,
        "mute" => 0xAD,
        "volume_down" => 0xAE,
        "volume_up" => 0xAF,
        _ => return None,
    })
}
pub fn parse_hotkey(text: &str) -> Result<Vec<u16>> {
    let mut keys = Vec::new();
    for part in text.split('+') {
        let p = part.trim().to_uppercase();
        let vk = match p.as_str() {
            "CTRL" | "CONTROL" => 0x11,
            "SHIFT" => 0x10,
            "ALT" => 0x12,
            "WIN" => 0x5B,
            "ENTER" => 0x0D,
            "TAB" => 9,
            "ESC" => 0x1B,
            "SPACE" => 0x20,
            "BACKSPACE" => 8,
            "DELETE" => 0x2E,
            "INSERT" => 0x2D,
            "HOME" => 0x24,
            "END" => 0x23,
            "PAGEUP" => 0x21,
            "PAGEDOWN" => 0x22,
            "LEFT" => 0x25,
            "UP" => 0x26,
            "RIGHT" => 0x27,
            "DOWN" => 0x28,
            "PLUS" => 0xBB,
            "MINUS" => 0xBD,
            "COMMA" => 0xBC,
            "PERIOD" => 0xBE,
            "SLASH" => 0xBF,
            "BACKSLASH" => 0xDC,
            "SEMICOLON" => 0xBA,
            "QUOTE" => 0xDE,
            "LBRACKET" => 0xDB,
            "RBRACKET" => 0xDD,
            "BACKTICK" => 0xC0,
            _ if p.len() == 1 && p.as_bytes()[0].is_ascii_alphanumeric() => p.as_bytes()[0] as u16,
            _ if p.starts_with('F') => {
                let n = p[1..].parse::<u16>().unwrap_or(0);
                if !(1..=24).contains(&n) {
                    bail!("Touche inconnue : {p}");
                }
                0x70 + n - 1
            }
            _ => bail!("Touche inconnue : {p}"),
        };
        if keys.contains(&vk) {
            bail!("Touche répétée : {p}");
        }
        keys.push(vk);
    }
    if keys.is_empty() || keys.len() > 8 {
        bail!("Raccourci invalide");
    }
    Ok(keys)
}
pub fn validate_key(key: &Key, config: &Config) -> Result<()> {
    if !(1..=1000).contains(&key.repeat) || key.steps.len() > 256 {
        bail!("Limite : 1000 répétitions, 256 étapes");
    }
    if key.steps.iter().any(|s| {
        matches!(
            s,
            Step::Page { .. }
                | Step::Folder { .. }
                | Step::Back
                | Step::PageNext
                | Step::PagePrevious
        )
    }) && (key.steps.len() != 1 || key.repeat != 1)
    {
        bail!("Une touche de navigation contient une seule action, sans répétition");
    }
    for s in &key.steps {
        match s {
            Step::Delay { ms } if *ms > 3_600_000 => bail!("Pause supérieure à une heure"),
            Step::Hotkey { keys } => {
                parse_hotkey(keys)?;
            }
            Step::Text { text } if text.len() > 100_000 => bail!("Texte trop long"),
            Step::Launch { program, args } if program.trim().is_empty() || args.len() > 128 => {
                bail!("Application ou arguments invalides")
            }
            Step::Url { url }
                if !(url.starts_with("https://") || url.starts_with("http://"))
                    || url.chars().any(char::is_control) =>
            {
                bail!("Adresse HTTP/HTTPS requise")
            }
            Step::WaitWindow {
                title, timeout_ms, ..
            } if title.trim().is_empty() || !(100..=3_600_000).contains(timeout_ms) => {
                bail!("Titre de fenêtre ou délai invalide")
            }
            Step::Media { command } if media_vk(command).is_none() => {
                bail!("Commande média inconnue")
            }
            Step::Profile { name } if !config.profiles.iter().any(|p| p.name == *name) => {
                bail!("Profil introuvable : {name}")
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pages_and_nested_folders_preserve_contents_and_return_to_origin() {
        let mut c = Config::default();
        c.profiles[0]
            .pages
            .push(Page::new("p2".into(), "Page 2".into(), false));
        c.profiles[0]
            .pages
            .push(Page::new("folder".into(), "Dossier".into(), true));
        c.profiles[0]
            .pages
            .push(Page::new("nested".into(), "Sous-dossier".into(), true));
        c.navigate(&Step::PageNext).unwrap();
        assert_eq!(c.page, "p2");
        c.keys_mut()[1].label = "Sur la page 2".into();
        c.navigate(&Step::Folder {
            id: "folder".into(),
        })
        .unwrap();
        assert_eq!(c.keys()[0].steps, vec![Step::Back]);
        c.navigate(&Step::Folder {
            id: "nested".into(),
        })
        .unwrap();
        c.navigate(&Step::Back).unwrap();
        assert_eq!(c.page, "folder");
        c.navigate(&Step::Back).unwrap();
        assert_eq!(c.page, "p2");
        assert_eq!(c.keys()[1].label, "Sur la page 2");
        c.navigate(&Step::PagePrevious).unwrap();
        assert!(c.page.is_empty());
        assert_eq!(c.keys()[1].label, "Lecture");
        c.validate().unwrap();
        let restored: Config = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.profiles[0].pages.len(), 3);
    }
    #[test]
    fn rejects_dangling_folders_cross_profile_targets_and_mixed_navigation() {
        let mut c = Config::default();
        c.profiles.push(Profile::new("Other".into()));
        c.profiles[1]
            .pages
            .push(Page::new("foreign".into(), "Folder".into(), true));
        c.profiles[0].keys[0].steps = vec![Step::Folder {
            id: "foreign".into(),
        }];
        assert!(c.validate().is_err());
        c.profiles[0].keys[0].steps = vec![
            Step::PageNext,
            Step::Text {
                text: "side effect".into(),
            },
        ];
        assert!(c.validate().is_err());
        c.profiles[0].keys[0].steps = vec![Step::Back];
        assert!(c.validate().is_ok());
        let previous = c.page.clone();
        assert!(c
            .navigate(&Step::Folder {
                id: "missing".into()
            })
            .is_err());
        assert_eq!(c.page, previous);
    }
    #[test]
    fn hotkeys_reject_unknown_and_duplicates() {
        assert!(parse_hotkey("CTRL+K").is_ok());
        assert!(parse_hotkey("CTRL+UNKNOWN").is_err());
        assert!(parse_hotkey("CTRL+CTRL").is_err());
        assert!(parse_hotkey("F25").is_err());
    }
    #[test]
    fn imported_config_cannot_bypass_limits() {
        let mut c = Config::default();
        c.profiles[0].keys[0].repeat = 0;
        assert!(c.validate().is_err());
        c.profiles[0].keys[0].repeat = 1;
        c.profiles[0].keys[0].steps.push(Step::Profile {
            name: "absent".into(),
        });
        assert!(c.validate().is_err());
    }
}
