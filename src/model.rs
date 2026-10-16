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
}
impl Profile {
    pub fn new(name: String) -> Self {
        Self {
            name,
            keys: vec![Key::default(); 15],
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
            schema: 1,
            brightness: 70,
            active: 0,
            profiles: vec![profile],
            flip_input_rows: false,
            image_size: 126,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.schema != 1 {
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
            if p.keys.len() != 15 {
                bail!("Chaque profil doit contenir 15 touches");
            }
            for key in &p.keys {
                validate_key(key, self)?;
            }
        }
        Ok(())
    }
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
