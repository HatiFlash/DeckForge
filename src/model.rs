use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    Disabled { step: Box<Step> },
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
    AudioSwitch {
        first: String,
        second: String,
        input: bool,
        #[serde(default)]
        first_label: String,
        #[serde(default)]
        second_label: String,
        #[serde(default)]
        first_icon: String,
        #[serde(default)]
        second_icon: String,
    },
    IfWindow {
        title: String,
        then_steps: Vec<Step>,
        else_steps: Vec<Step>,
    },
    Repeat {
        count: u32,
        steps: Vec<Step>,
    },
    SetVariable {
        name: String,
        value: String,
    },
    IfVariable {
        name: String,
        value: String,
        then_steps: Vec<Step>,
        else_steps: Vec<Step>,
    },
    AutoHotkey {
        executable: String,
        script: String,
        args: Vec<String>,
    },
    PlaySound { path: String, volume: u8 },
    StopSounds,
    Toggle { a: Vec<Step>, b: Vec<Step> },
    UdpSend { host: String, port: u16, text: String },
    HttpRequest { method: String, url: String, body: String, content_type: String },
    D6Brightness { mode: String, value: u8 },
    D6Sleep,
    CloseWindow,
    MouseClick { button: String, double: bool },
    MouseMove { x: i32, y: i32, relative: bool },
    MouseScroll { amount: i32 },
    ObsScene { name: String },
    ObsRecord { mode: String },
    ObsStream { mode: String },
    ObsVirtualCam { mode: String },
    ObsReplay { mode: String },
    ObsMute { input: String, mode: String },
    ObsSourceVisibility { scene: String, source: String, mode: String },
    Back,
    PageNext,
    PagePrevious,
}
impl Step {
    pub fn is_disabled(&self) -> bool { matches!(self, Self::Disabled { .. }) }
    pub fn active(&self) -> &Self {
        match self { Self::Disabled { step } => step.active(), other => other }
    }
    pub fn active_mut(&mut self) -> &mut Self {
        match self { Self::Disabled { step } => step.active_mut(), other => other }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Disabled { step } => step.name(),
            Self::Delay { .. } => "Attendre",
            Self::Hotkey { .. } => "Raccourci",
            Self::Text { .. } => "Saisir du texte",
            Self::Launch { .. } => "Lancer une application",
            Self::Url { .. } => "Ouvrir un site",
            Self::WaitWindow { .. } => "Attendre une fenêtre",
            Self::Media { .. } => "Média / volume",
            Self::Profile { .. } => "Changer de profil",
            Self::Page { .. } => "Aller à une page",
            Self::Folder { .. } => "Ouvrir un dossier",
            Self::AudioSwitch { .. } => "Basculer le périphérique audio",
            Self::IfWindow { .. } => "Si une fenêtre est ouverte",
            Self::Repeat { .. } => "Répéter des blocs",
            Self::SetVariable { .. } => "Définir une variable",
            Self::IfVariable { .. } => "Si une variable correspond",
            Self::AutoHotkey { .. } => "Script AutoHotkey",
            Self::MouseClick { .. } => "Clic souris",
            Self::MouseMove { .. } => "Déplacer la souris",
            Self::MouseScroll { .. } => "Défilement souris",
            Self::PlaySound { .. } => "Jouer un son",
            Self::StopSounds => "Arrêter les sons",
            Self::Toggle { .. } => "Basculer entre deux actions",
            Self::UdpSend { .. } => "Envoyer en UDP",
            Self::HttpRequest { .. } => "Requête HTTP",
            Self::D6Brightness { .. } => "Luminosité du D6",
            Self::D6Sleep => "Mettre le D6 en veille",
            Self::CloseWindow => "Fermer la fenêtre active",
            Self::ObsScene { .. } => "OBS : scène",
            Self::ObsRecord { .. } => "OBS : enregistrement",
            Self::ObsStream { .. } => "OBS : diffusion",
            Self::ObsVirtualCam { .. } => "OBS : caméra virtuelle",
            Self::ObsReplay { .. } => "OBS : replay",
            Self::ObsMute { .. } => "OBS : couper une entrée",
            Self::ObsSourceVisibility { .. } => "OBS : visibilité de source",
            Self::Back => "Retour au dossier précédent",
            Self::PageNext => "Page suivante",
            Self::PagePrevious => "Page précédente",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PluginBinding {
    pub plugin: String,
    pub action: String,
    #[serde(default)] pub settings: serde_json::Value,
    #[serde(default)] pub state: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Key {
    pub label: String,
    pub color: [u8; 3],
    #[serde(default = "default_background")]
    pub background: bool,
    pub image: String,
    pub repeat: u32,
    #[serde(default = "default_title_size", skip_serializing_if = "is_default_title_size")]
    pub title_size: u8,
    #[serde(default = "default_icon_scale", skip_serializing_if = "is_default_icon_scale")]
    pub icon_scale: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginBinding>,
    pub steps: Vec<Step>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub long_steps: Vec<Step>,
    #[serde(skip)]
    pub display_kind: Option<usize>,
    #[serde(skip)]
    pub runtime_badge: Option<String>,
    #[serde(skip)]
    pub runtime_id: Option<String>,
}
impl Default for Key {
    fn default() -> Self {
        Self {
            label: String::new(),
            color: [35, 44, 60],
            background: true,
            image: String::new(),
            repeat: 1,
            title_size: 14,
            icon_scale: 100,
            plugin: None,
            steps: vec![],
            long_steps: vec![],
            display_kind: None,
            runtime_badge: None,
            runtime_id: None,
        }
    }
}
fn default_long_press_ms() -> u64 { 500 }
fn default_animation_fps() -> u8 { 12 }
fn default_icon_scale() -> u8 { 100 }
fn is_default_icon_scale(value: &u8) -> bool { *value == 100 }
fn default_title_size() -> u8 { 14 }
fn is_default_title_size(value: &u8) -> bool { *value == 14 }
fn default_background() -> bool {
    true
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub keys: Vec<Key>,
    #[serde(default)]
    pub pages: Vec<Page>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<String>,
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
            apps: vec![],
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
    #[serde(default)]
    pub obs: ObsSettings,
    pub brightness: u8,
    #[serde(default)]
    pub auto_profile: bool,
    #[serde(default)]
    pub sleep_after_min: u16,
    #[serde(default)]
    pub sleep_brightness: u8,
    #[serde(default = "default_long_press_ms")]
    pub long_press_ms: u64,
    #[serde(default = "default_background")]
    pub d6_animations: bool,
    #[serde(default = "default_animation_fps")]
    pub animation_fps: u8,
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObsSettings {
    #[serde(default = "obs_host")] pub host: String,
    #[serde(default = "obs_port")] pub port: u16,
    #[serde(default)] pub password: String,
}
fn obs_host() -> String { "127.0.0.1".into() }
fn obs_port() -> u16 { 4455 }
impl Default for ObsSettings {
    fn default() -> Self { Self { host: obs_host(), port: obs_port(), password: String::new() } }
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
            obs: ObsSettings::default(),
            brightness: 70,
            auto_profile: false,
            sleep_after_min: 0,
            sleep_brightness: 0,
            long_press_ms: 500,
            d6_animations: true,
            animation_fps: 12,
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
    /// Drafts retain incomplete bindings; only a sanitized snapshot reaches USB.
    pub fn validate_structure(&self) -> Result<()> {
        let mut shape = self.clone();
        for p in &mut shape.profiles {
            for keys in std::iter::once(&mut p.keys).chain(p.pages.iter_mut().map(|p| &mut p.keys))
            {
                for key in keys {
                    key.steps.clear();
                    key.long_steps.clear();
                    key.repeat = 1;
                }
            }
        }
        shape.validate()
    }
    pub fn binding_errors(&self) -> Vec<String> {
        let mut errors = vec![];
        for p in &self.profiles {
            for (page, keys) in std::iter::once(("Page 1", &p.keys))
                .chain(p.pages.iter().map(|p| (p.name.as_str(), &p.keys)))
            {
                for (i, key) in keys.iter().enumerate() {
                    if let Err(e) = validate_key_in_profile(key, self, p) {
                        errors.push(format!(
                            "{} / {} / touche {:02} « {} » : {e}",
                            p.name,
                            page,
                            i + 1,
                            key.label
                        ));
                    }
                }
            }
        }
        errors
    }
    pub fn deployment(&self) -> Result<Self> {
        self.validate_structure()?;
        let mut next = self.clone();
        for (pi, p) in next.profiles.iter_mut().enumerate() {
            for keys in std::iter::once(&mut p.keys).chain(p.pages.iter_mut().map(|p| &mut p.keys))
            {
                for key in keys {
                    if validate_key_in_profile(key, self, &self.profiles[pi]).is_err() {
                        key.steps.clear();
                        key.long_steps.clear();
                        key.repeat = 1;
                        key.image.clear();
                        key.color = [110, 44, 49];
                        key.label = "À compléter".into();
                    }
                }
            }
        }
        next.validate()?;
        Ok(next)
    }
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(self.obs.host == "127.0.0.1" || self.obs.host == "localhost", "OBS doit être local");
        anyhow::ensure!(self.obs.port != 0 && self.obs.password.len() <= 4096, "Réglages OBS invalides");
        if ![1, 2].contains(&self.schema) {
            bail!("Version de configuration non prise en charge");
        }
        if self.profiles.is_empty()
            || self.profiles.len() > 64
            || self.active >= self.profiles.len()
        {
            bail!("Profils invalides (1 à 64)");
        }
        if !(250..=2000).contains(&self.long_press_ms) {
            bail!("Seuil de l’appui long invalide (250 à 2000 ms)");
        }
        if self.sleep_after_min > 240 || self.sleep_brightness > 100 {
            bail!("Réglages de veille invalides");
        }
        if !(1..=15).contains(&self.animation_fps) || self.brightness > 100 || ![112, 126].contains(&self.image_size) {
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
    for step in key.steps.iter().chain(&key.long_steps) {
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
        "browser_back" => 0xA6,
        "browser_forward" => 0xA7,
        "browser_refresh" => 0xA8,
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
    if text.trim().is_empty() {
        bail!("Raccourci clavier vide : enregistre une combinaison");
    }
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
            "BROWSER_BACK" => 0xA6,
            "BROWSER_FORWARD" => 0xA7,
            "BROWSER_REFRESH" => 0xA8,
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
    if !key.long_steps.is_empty() {
        let mut long = key.clone();
        long.steps = std::mem::take(&mut long.long_steps);
        validate_key(&long, config)?;
    }
    anyhow::ensure!((10..=20).contains(&key.title_size), "Taille du titre invalide");
    anyhow::ensure!((20..=200).contains(&key.icon_scale), "Taille de l’icône invalide");
    if !(1..=1000).contains(&key.repeat) || key.steps.len() > 256 {
        bail!("Limite : 1000 répétitions, 256 étapes");
    }
    if key.steps.iter().filter(|s| !s.is_disabled()).any(|s| {
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
    let mut total = 0;
    validate_tree(&key.steps, 0, &mut total)?;
    for s in &key.steps {
        if s.is_disabled() { continue; }
        match s {
            Step::MouseClick { button, .. } if !["left", "right", "middle"].contains(&button.as_str()) => bail!("Bouton souris invalide"),
            Step::MouseMove { x, y, .. } if !(-32768..=32767).contains(x) || !(-32768..=32767).contains(y) => bail!("Position souris hors limites"),
            Step::MouseScroll { amount } if !(-100..=100).contains(amount) => bail!("Défilement souris hors limites"),
            Step::PlaySound { path, volume } if path.trim().is_empty() || path.len() > 1024 || path.contains('"') || path.chars().any(char::is_control) || !["wav", "mp3"].iter().any(|ext| std::path::Path::new(path).extension().is_some_and(|e| e.eq_ignore_ascii_case(ext))) || *volume > 100 => bail!("Son WAV/MP3 ou volume invalide"),
            Step::StopSounds => {},
            Step::Toggle { a, b } => { for steps in [a, b] { validate_key(&Key { steps: steps.clone(), ..Key::default() }, config)?; } },
            Step::UdpSend { host, port, text } if !valid_udp_host(host) || *port == 0 || text.len() > 65507 => bail!("Destination UDP ou message invalide"),
            Step::HttpRequest { method, url, body, content_type } => validate_http(method, url, body, content_type)?,
            Step::D6Brightness { mode, value } if !["set", "up", "down"].contains(&mode.as_str()) || *value > 100 => bail!("Mode ou luminosité du D6 invalide"),
            Step::D6Sleep => {},
            Step::CloseWindow => {},
            Step::ObsScene { name } if name.trim().is_empty() || name.len() > 256 => bail!("Scène OBS invalide"),
            Step::ObsMute { input, mode } if input.trim().is_empty() || !obs_mode(mode, &["toggle", "mute", "unmute"]) => bail!("Entrée OBS ou mode invalide"),
            Step::ObsSourceVisibility { scene, source, mode } if scene.trim().is_empty() || source.trim().is_empty() || !obs_mode(mode, &["toggle", "show", "hide"]) => bail!("Source OBS ou mode invalide"),
            Step::ObsRecord { mode } if !obs_mode(mode, &["toggle", "start", "stop", "pause"]) => bail!("Mode OBS invalide"),
            Step::ObsStream { mode } | Step::ObsVirtualCam { mode } if !obs_mode(mode, &["toggle", "start", "stop"]) => bail!("Mode OBS invalide"),
            Step::ObsReplay { mode } if !obs_mode(mode, &["toggle", "start", "stop", "save"]) => bail!("Mode replay invalide"),
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
            Step::AudioSwitch { first, second, .. }
                if first.is_empty() || second.is_empty() || first == second =>
            {
                bail!("Choisis deux périphériques audio différents")
            }
            Step::IfWindow { title, .. } if title.trim().is_empty() => {
                bail!("Titre de fenêtre vide")
            }
            Step::SetVariable { name, value } | Step::IfVariable { name, value, .. }
                if name.is_empty()
                    || name.len() > 64
                    || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    || value.len() > 4096 =>
            {
                bail!("Variable invalide (nom : lettres, chiffres ou _)")
            }
            Step::AutoHotkey {
                executable,
                script,
                args,
            } if executable.trim().is_empty()
                || !executable.to_ascii_lowercase().ends_with(".exe")
                || script.trim().is_empty()
                || script.len() > 200_000
                || args.len() > 128 =>
            {
                bail!("Choisis AutoHotkey v2 et renseigne le script")
            }
            Step::IfWindow {
                then_steps,
                else_steps,
                ..
            }
            | Step::IfVariable {
                then_steps,
                else_steps,
                ..
            } => {
                for steps in [then_steps, else_steps] {
                    validate_key(
                        &Key {
                            steps: steps.clone(),
                            ..Key::default()
                        },
                        config,
                    )?;
                }
            }
            Step::Repeat { steps, .. } => validate_key(
                &Key {
                    steps: steps.clone(),
                    ..Key::default()
                },
                config,
            )?,
            _ => {}
        }
    }
    Ok(())
}
pub fn d6_brightness(current: u8, mode: &str, value: u8) -> u8 {
    match mode { "set" => value, "up" => current.saturating_add(value).min(100), "down" => current.saturating_sub(value), _ => current }
}
pub fn valid_udp_host(host: &str) -> bool {
    !host.is_empty() && host.len() <= 253 && host.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
        && !host.starts_with('-') && !host.ends_with('-')
}
pub fn validate_http(method: &str, url: &str, body: &str, content_type: &str) -> Result<()> {
    if url.starts_with("https://") { bail!("HTTPS non pris en charge sans bibliothèque TLS"); }
    let rest = url.strip_prefix("http://").ok_or_else(|| anyhow::anyhow!("Adresse HTTP requise"))?;
    let authority = rest.split('/').next().unwrap_or("");
    let (host, port) = authority.rsplit_once(':').unwrap_or((authority, "80"));
    anyhow::ensure!(valid_udp_host(host) && port.parse::<u16>().is_ok_and(|n| n != 0)
        && url.len() <= 2048 && !url.chars().any(char::is_control)
        && ["GET", "POST", "PUT"].contains(&method) && body.len() <= 65536
        && content_type.len() <= 128 && !content_type.chars().any(|c| c.is_control() || c == '\r' || c == '\n'), "Requête HTTP invalide");
    Ok(())
}
fn obs_mode(mode: &str, valid: &[&str]) -> bool { valid.contains(&mode) }
fn validate_tree(steps: &[Step], depth: usize, total: &mut usize) -> Result<()> {
    anyhow::ensure!(depth <= 8, "8 niveaux de blocs maximum");
    *total += steps.len();
    anyhow::ensure!(*total <= 256, "256 blocs maximum par macro");
    for s in steps {
        if s.is_disabled() { continue; }
        match s {
            Step::IfWindow {
                then_steps,
                else_steps,
                ..
            }
            | Step::IfVariable {
                then_steps,
                else_steps,
                ..
            } => {
                validate_tree(then_steps, depth + 1, total)?;
                validate_tree(else_steps, depth + 1, total)?;
            }
            Step::Toggle { a, b } => { validate_tree(a, depth + 1, total)?; validate_tree(b, depth + 1, total)?; }
            Step::Repeat { count, steps } => {
                anyhow::ensure!((1..=100).contains(count), "1 à 100 répétitions par bloc");
                validate_tree(steps, depth + 1, total)?;
            }
            Step::Page { .. }
            | Step::Folder { .. }
            | Step::Back
            | Step::PageNext
            | Step::PagePrevious
            | Step::Profile { .. }
                if depth > 0 =>
            {
                bail!("La navigation doit être une action principale")
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
    fn d6_lighting_steps_validate_and_calculate() {
        assert_eq!(d6_brightness(95, "up", 20), 100);
        assert_eq!(d6_brightness(4, "down", 20), 0);
        assert_eq!(d6_brightness(90, "set", 30), 30);
        let c=Config::default();
        for step in [Step::D6Brightness { mode: "bad".into(), value: 10 }, Step::D6Brightness { mode: "set".into(), value: 101 }] {
            assert!(validate_key(&Key { steps: vec![step], ..Key::default() }, &c).is_err());
        }
        for step in [Step::D6Sleep, Step::D6Brightness { mode: "up".into(), value: 10 }] {
            let value = serde_json::to_value(&step).unwrap();
            assert_eq!(serde_json::from_value::<Step>(value).unwrap(), step);
        }
    }
    #[test]
    fn browser_virtual_keys_are_distinct_and_validated() {
        for (name, vk) in [("BROWSER_BACK", 0xA6), ("BROWSER_FORWARD", 0xA7), ("BROWSER_REFRESH", 0xA8)] {
            assert_eq!(parse_hotkey(name).unwrap(), vec![vk]);
        }
        assert!(parse_hotkey("BROWSER_UNKNOWN").is_err());
    }
    #[test]
    fn long_press_config_and_actions_keep_legacy_compatible() {
        let mut c = Config::default();
        let legacy = serde_json::to_value(&c).unwrap();
        assert!(legacy.get("long_press_ms").is_some());
        let mut legacy = legacy;
        legacy.as_object_mut().unwrap().remove("long_press_ms");
        let key = legacy["profiles"][0]["keys"][0].as_object_mut().unwrap();
        key.remove("long_steps");
        key.remove("title_size");
        key.remove("icon_scale");
        let restored: Config = serde_json::from_value(legacy).unwrap();
        assert_eq!(restored.long_press_ms, 500);
        assert_eq!(restored.keys()[0].title_size, 14);
        assert_eq!(restored.keys()[0].icon_scale, 100);
        assert!(restored.d6_animations);
        assert_eq!(restored.animation_fps, 12);
        assert!(restored.keys()[0].long_steps.is_empty());
        c.long_press_ms = 249;
        assert!(c.validate().is_err());
        c.long_press_ms = 2000;
        assert!(c.validate().is_ok());
        c.keys_mut()[0].long_steps = vec![Step::Page { id: "missing".into() }];
        assert!(c.validate().is_err());
        c.keys_mut()[0].long_steps = vec![Step::Hotkey { keys: String::new() }];
        assert!(c.validate().is_err());
    }
    #[test]
    fn disabled_step_roundtrip_and_legacy_json_remain_compatible() {
        let legacy = r#"{"label":"Legacy","color":[1,2,3],"image":"","repeat":1,"steps":[{"type":"delay","ms":5}]}"#;
        let mut key: Key = serde_json::from_str(legacy).unwrap();
        assert!(key.background);
        assert!(!key.steps[0].is_disabled());
        key.steps[0] = Step::Disabled { step: Box::new(key.steps[0].clone()) };
        let json = serde_json::to_string(&key).unwrap();
        assert!(json.contains("\"type\":\"disabled\""));
        let loaded: Key = serde_json::from_str(&json).unwrap();
        assert!(loaded.steps[0].is_disabled());
        assert!(validate_key(&loaded, &Config::default()).is_ok());
        let invalid = Key { steps: vec![Step::Disabled { step: Box::new(Step::Hotkey { keys: String::new() }) }], ..Key::default() };
        assert!(validate_key(&invalid, &Config::default()).is_ok());
    }

    #[test]
    fn nested_validation_rejects_bad_keys_deep_trees_and_nested_navigation() {
        let c = Config::default();
        let mut key = Key {
            steps: vec![Step::Repeat {
                count: 2,
                steps: vec![Step::Hotkey {
                    keys: String::new(),
                }],
            }],
            ..Key::default()
        };
        assert!(validate_key(&key, &c).is_err());
        key.steps = vec![Step::Repeat {
            count: 2,
            steps: vec![Step::PageNext],
        }];
        assert!(validate_key(&key, &c).is_err());
        key.steps = vec![Step::Delay { ms: 10 }];
        for _ in 0..10 {
            key.steps = vec![Step::Repeat {
                count: 2,
                steps: key.steps,
            }];
        }
        assert!(validate_key(&key, &c).is_err());
    }
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

#[cfg(test)]
mod native_sound_tests {
    use super::*;
    #[test]
    fn sound_json_validation_and_export_safety() {
        let c = Config::default();
        let good = Step::PlaySound { path: "C:\\sons\\été.mp3".into(), volume: 65 };
        let json = serde_json::to_string(&good).unwrap();
        assert_eq!(serde_json::from_str::<Step>(&json).unwrap(), good);
        for step in [good, Step::StopSounds] {
            let key = Key { steps: vec![step], ..Key::default() };
            validate_key(&key, &c).unwrap();
            assert!(crate::automation::export(&key).is_err());
        }
        for path in ["", "voice.exe", "x\"y.wav"] {
            assert!(validate_key(&Key { steps: vec![Step::PlaySound { path: path.into(), volume: 50 }], ..Key::default() }, &c).is_err());
        }
        assert!(validate_key(&Key { steps: vec![Step::PlaySound { path: "ok.wav".into(), volume: 101 }], ..Key::default() }, &c).is_err());
    }
}
#[cfg(test)]
mod native_close_tests {
    use super::*;
    #[test]
    fn close_window_is_valid_and_serialized() {
        let step = Step::CloseWindow;
        assert_eq!(serde_json::to_string(&step).unwrap(), r#"{"type":"close_window"}"#);
        validate_key(&Key { steps: vec![step], ..Key::default() }, &Config::default()).unwrap();
    }
}

#[cfg(test)]
mod native_mouse_tests {
    use super::*;
    #[test]
    fn mouse_actions_validate_and_roundtrip() {
        let c = Config::default();
        for step in [Step::MouseClick { button: "left".into(), double: true }, Step::MouseMove { x: 14, y: -5, relative: true }, Step::MouseScroll { amount: -3 }] {
            let json = serde_json::to_string(&step).unwrap();
            assert_eq!(serde_json::from_str::<Step>(&json).unwrap(), step);
            validate_key(&Key { steps: vec![step], ..Key::default() }, &c).unwrap();
        }
        for invalid in [Step::MouseClick { button: "other".into(), double: false }, Step::MouseScroll { amount: 101 }, Step::MouseMove { x: 99999, y: 0, relative: false }] {
            assert!(validate_key(&Key { steps: vec![invalid], ..Key::default() }, &c).is_err());
        }
    }
}

#[cfg(test)] mod plugin_compatibility_tests {
    use super::*;
    #[test] fn legacy_key_and_plugin_binding_roundtrip() {
        let original = serde_json::to_value(Key::default()).unwrap();
        assert!(original.get("plugin").is_none());
        let mut key: Key = serde_json::from_value(original).unwrap();
        assert!(key.plugin.is_none() && key.background);
        key.plugin = Some(PluginBinding {plugin:"example.plugin".into(), action:"example.action".into(),settings:serde_json::json!({"mode":1}),state:2});
        let loaded: Key = serde_json::from_value(serde_json::to_value(&key).unwrap()).unwrap();
        assert_eq!(loaded.plugin.unwrap().settings["mode"],1);
    }
}
