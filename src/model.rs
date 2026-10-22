use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    Disabled { step: Box<Step> },
    PluginAction {
        plugin: String,
        action: String,
        #[serde(default = "empty_plugin_settings")] settings: serde_json::Value,
        #[serde(default)] label: String,
    },
    Delay {
        ms: u64,
    },
    Hotkey {
        keys: String,
    },
    KeyHold {
        #[serde(default = "default_hold_keys")] keys: String,
        #[serde(default = "default_hold_action")] action: String,
        #[serde(default)] hold_ms: u64,
    },
    Text {
        text: String,
    },
    Launch {
        program: String,
        args: Vec<String>,
        #[serde(default)] focus_if_running: bool,
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
    AppVolume {
        #[serde(default = "default_app_target")]
        target: String,
        #[serde(default = "default_app_action")]
        action: String,
        #[serde(default = "default_app_step")]
        step: u8,
        #[serde(default)]
        level: u8,
        #[serde(default = "default_background")]
        include_children: bool,
    },
    WindowOp {
        #[serde(default = "default_app_target")] target: String,
        #[serde(default)] value: String,
        #[serde(default = "default_window_op")] op: String,
        #[serde(default)] x: i32,
        #[serde(default)] y: i32,
        #[serde(default = "default_window_width")] w: i32,
        #[serde(default = "default_window_height")] h: i32,
        #[serde(default = "default_window_monitor")] monitor: u32,
    },
    Clipboard {
        #[serde(default = "default_clipboard_op")] op: String,
        #[serde(default)] value: String,
        #[serde(default = "default_clipboard_name")] name: String,
    },
    DeviceVolume {
        #[serde(default)] input: bool,
        #[serde(default = "default_device")] device: String,
        #[serde(default = "default_app_action")] action: String,
        #[serde(default = "default_app_step")] step: u8,
        #[serde(default = "default_device_level")] level: u8,
    },
    Notify {
        #[serde(default = "default_notify_title")] title: String,
        #[serde(default)] message: String,
    },
    WaitUntil {
        #[serde(default)] condition: crate::conditions::Condition,
        #[serde(default = "default_wait_timeout")] timeout_ms: u64,
        #[serde(default = "default_wait_poll")] poll_ms: u64,
    },
    IfCondition {
        #[serde(default)] condition: crate::conditions::Condition,
        #[serde(default)] then_steps: Vec<Step>,
        #[serde(default)] else_steps: Vec<Step>,
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
    Math {
        #[serde(default = "default_math_name")] name: String,
        #[serde(default = "default_math_op")] op: String,
        #[serde(default = "default_math_value")] value: String,
        #[serde(default)] min: f64,
        #[serde(default = "default_math_max")] max: f64,
        #[serde(default)] persistent: bool,
    },
    TextOp {
        #[serde(default = "default_text_name")] name: String,
        #[serde(default = "default_text_op")] op: String,
        #[serde(default)] value: String,
        #[serde(default)] find: String,
        #[serde(default)] persistent: bool,
    },
    SetVariable {
        #[serde(default = "default_set_name")] name: String,
        #[serde(default)] value: String,
        #[serde(default)] persistent: bool,
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
    MouseClick { #[serde(default = "default_mouse_button")] button: String, #[serde(default)] double: bool,
        #[serde(default)] x: Option<i32>, #[serde(default)] y: Option<i32>,
        #[serde(default = "default_mouse_origin")] relative_to: String },
    MouseMove { #[serde(default)] x: i32, #[serde(default)] y: i32,
        #[serde(default = "default_mouse_origin")] relative_to: String,
        // Anciennes configurations : relative=true signifiait déplacement depuis le curseur.
        #[serde(default, skip_serializing_if = "is_false")] relative: bool },
    MouseScroll { #[serde(default = "default_scroll_amount")] amount: i32,
        #[serde(default)] horizontal: bool },
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
fn empty_plugin_settings() -> serde_json::Value { serde_json::json!({}) }
fn valid_plugin_identifier(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && id.trim() == id
        && !id.chars().any(char::is_control)
}
fn default_mouse_button() -> String { "left".into() }
fn default_mouse_origin() -> String { "screen".into() }
fn default_scroll_amount() -> i32 { 1 }
fn is_false(value: &bool) -> bool { !*value }
fn valid_mouse_origin(value: &str) -> bool { ["screen", "window", "cursor"].contains(&value) }
fn mouse_coordinate_valid(value: i32) -> bool { (-32768..=32767).contains(&value) }
fn default_hold_keys() -> String { "CTRL".into() }
fn default_hold_action() -> String { "down".into() }
fn default_text_name() -> String { "texte".into() }
fn default_text_op() -> String { "upper".into() }
fn valid_text_op(op: &str) -> bool { ["set", "append", "prepend", "upper", "lower", "trim", "replace", "length"].contains(&op) }
fn default_clipboard_op() -> String { "copy_text".into() }
fn default_clipboard_name() -> String { "texte".into() }
fn default_math_name() -> String { "compteur".into() }
fn default_math_op() -> String { "add".into() }
fn default_math_value() -> String { "1".into() }
fn default_math_max() -> f64 { 100.0 }
fn valid_math_name(name: &str) -> bool { !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') }
fn default_set_name() -> String { "mode".into() }
fn default_notify_title() -> String { "NovyDeck".into() }
fn default_wait_timeout() -> u64 { 5000 }
fn default_wait_poll() -> u64 { 200 }
fn default_device() -> String { "default".into() }
fn default_device_level() -> u8 { 50 }
fn default_window_op() -> String { "maximize".into() }
fn default_window_width() -> i32 { 800 }
fn default_window_height() -> i32 { 600 }
fn default_window_monitor() -> u32 { 2 }
fn valid_window_op(target: &str, value: &str, op: &str, x: i32, y: i32, w: i32, h: i32, monitor: u32) -> bool {
    ["foreground", "title", "exe"].contains(&target)
        && value.len() <= 260 && !value.chars().any(char::is_control)
        && (target == "foreground" || !value.trim().is_empty() && value.trim() == value)
        && (target != "exe" || value.to_ascii_lowercase().ends_with(".exe") && !value.contains(['/', '\\']))
        && ["activate", "minimize", "maximize", "restore", "move", "resize", "move_to_monitor", "toggle_always_on_top", "always_on_top_on", "always_on_top_off", "close_others", "minimize_others"].contains(&op)
        && (!matches!(op, "close_others" | "minimize_others") || target == "foreground")
        && (-32768..=32767).contains(&x) && (-32768..=32767).contains(&y)
        && (1..=32767).contains(&w) && (1..=32767).contains(&h)
        && (1..=16).contains(&monitor)
}
fn default_app_target() -> String { "foreground".into() }
fn default_app_action() -> String { "up".into() }
fn default_app_step() -> u8 { 5 }
impl Step {
    pub fn text_op_summary(&self) -> Option<String> {
        let Self::TextOp { name, op, value, find, .. } = self else { return None };
        Some(match op.as_str() {
            "set" => format!("Définir {name} à « {} »", value.chars().take(40).collect::<String>()),
            "append" => format!("Ajouter « {} » à la fin de {name}", value.chars().take(40).collect::<String>()),
            "prepend" => format!("Ajouter « {} » au début de {name}", value.chars().take(40).collect::<String>()),
            "upper" => format!("Mettre {name} en majuscules"),
            "lower" => format!("Mettre {name} en minuscules"),
            "trim" => format!("Supprimer les espaces autour de {name}"),
            "replace" => format!("Remplacer « {} » dans {name}", find.chars().take(40).collect::<String>()),
            "length" => format!("Compter les caractères de {name}"),
            _ => "Opération de texte inconnue".into(),
        })
    }
    pub fn math_summary(&self) -> Option<String> {
        let Self::Math { name, op, value, min, max, .. } = self else { return None };
        Some(match op.as_str() {
            "set" => format!("Définir {name} à {value}"),
            "add" => format!("Ajouter {value} à {name}"),
            "subtract" => format!("Soustraire {value} de {name}"),
            "multiply" => format!("Multiplier {name} par {value}"),
            "divide" => format!("Diviser {name} par {value}"),
            "modulo" => format!("Reste de {name} divisé par {value}"),
            "min" => format!("Minimum de {name} et {value}"),
            "max" => format!("Maximum de {name} et {value}"),
            "random" => format!("Tirer {name} entre {min} et {max}"),
            _ => "Calcul inconnu".into(),
        })
    }
    pub fn device_volume_summary(&self) -> Option<String> {
        let Self::DeviceVolume { input, device, action, step, level } = self else { return None };
        let who = if *input { "microphone" } else { "sortie audio" };
        let target = if device == "default" { format!("{who} par défaut") } else { format!("{who} ({device})") };
        Some(match action.as_str() {
            "up" => format!("Augmenter le volume du {target} de {step} points"),
            "down" => format!("Diminuer le volume du {target} de {step} points"),
            "set" => format!("Régler le volume du {target} à {level} %"),
            "mute" => format!("Couper le son du {target}"),
            "unmute" => format!("Rétablir le son du {target}"),
            "toggle_mute" => format!("Basculer le muet du {target}"),
            _ => format!("Action de volume inconnue pour {target}"),
        })
    }
    pub fn app_volume_summary(&self) -> Option<String> {
        let Self::AppVolume { target, action, step, level, .. } = self else { return None };
        let who = if target == "foreground" { "l’application active".to_owned() } else { target.clone() };
        Some(match action.as_str() {
            "up" => format!("Augmenter le volume de {who} de {step} points"),
            "down" => format!("Diminuer le volume de {who} de {step} points"),
            "set" => format!("Régler le volume de {who} à {level} %"),
            "mute" => format!("Couper le son de {who}"),
            "unmute" => format!("Rétablir le son de {who}"),
            "toggle_mute" => format!("Basculer le muet de {who}"),
            _ => format!("Volume de {who} : action inconnue"),
        })
    }

    pub fn key_hold_summary(&self) -> Option<String> {
        let Self::KeyHold { keys, action, hold_ms } = self else { return None };
        Some(match (action.as_str(), hold_ms) {
            ("down", 0) => format!("Maintenir {keys} jusqu’au bloc Relâcher ou à la fin de la macro"),
            ("down", ms) => format!("Maintenir {keys} pendant {ms} ms"),
            ("up", _) => format!("Relâcher {keys}"),
            _ => format!("Action de maintien inconnue : {keys}"),
        })
    }
    pub fn window_op_summary(&self) -> Option<String> {
        let Self::WindowOp { target, value, op, x, y, w, h, monitor } = self else { return None };
        let who = if target == "foreground" { "la fenêtre active".to_owned() } else { format!("la fenêtre {value}") };
        Some(match op.as_str() {
            "activate" => format!("Activer {who}"), "minimize" => format!("Réduire {who}"),
            "maximize" => format!("Maximiser {who}"), "restore" => format!("Restaurer {who}"),
            "move" => format!("Déplacer {who} à {x}, {y}"),
            "resize" => format!("Redimensionner {who} à {w} × {h}"),
            "move_to_monitor" => format!("Déplacer {who} sur l’écran {monitor}"),
            "toggle_always_on_top" => format!("Basculer toujours au premier plan pour {who}"),
            "always_on_top_on" => format!("Garder {who} toujours au premier plan"),
            "always_on_top_off" => format!("Désactiver toujours au premier plan pour {who}"),
            "close_others" => format!("Fermer toutes les autres fenêtres (garder {who})"),
            "minimize_others" => format!("Réduire toutes les autres fenêtres (garder {who})"),
            _ => format!("Opération de fenêtre inconnue : {who}"),
        })
    }
    pub fn clipboard_summary(&self) -> Option<String> {
        let Self::Clipboard { op, value, name } = self else { return None };
        Some(match op.as_str() {
            "copy_text" => format!("Copier du texte : {}", value.chars().take(50).collect::<String>()),
            "read_to_variable" => format!("Lire le presse-papiers dans {name}"),
            "clear" => "Vider le presse-papiers".into(),
            "save" => "Sauvegarder le texte du presse-papiers".into(),
            "restore" => "Restaurer le texte sauvegardé du presse-papiers".into(),
            _ => "Opération de presse-papiers inconnue".into(),
        })
    }
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
            Self::PluginAction { .. } => "Action de plugin",
            Self::Delay { .. } => "Attendre",
            Self::Hotkey { .. } => "Raccourci",
            Self::KeyHold { .. } => "Maintenir une touche",
            Self::Text { .. } => "Saisir du texte",
            Self::Launch { .. } => "Lancer une application",
            Self::Url { .. } => "Ouvrir un site",
            Self::WaitWindow { .. } => "Attendre une fenêtre",
            Self::Media { .. } => "Média / volume",
            Self::Profile { .. } => "Changer de profil",
            Self::Page { .. } => "Aller à une page",
            Self::Folder { .. } => "Ouvrir un dossier",
            Self::AudioSwitch { .. } => "Basculer le périphérique audio",
            Self::AppVolume { .. } => "Volume d’une application",
            Self::WindowOp { .. } => "Opération sur une fenêtre",
            Self::Clipboard { .. } => "Presse-papiers",
            Self::DeviceVolume { .. } => "Volume d’un périphérique",
            Self::IfCondition { .. } => "Si… sinon",
            Self::WaitUntil { .. } => "Attendre que…",
            Self::Notify { .. } => "Notification",
            Self::IfWindow { .. } => "Si une fenêtre est ouverte",
            Self::Repeat { .. } => "Répéter des blocs",
            Self::SetVariable { .. } => "Définir une variable",
            Self::Math { .. } => "Calculer",
            Self::TextOp { .. } => "Texte",
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
            Self::D6Brightness { .. } => "Luminosité du périphérique",
            Self::D6Sleep => "Mettre le périphérique en veille",
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode { #[default] Ignore, Restart, Queue }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Key {
    pub label: String,
    pub color: [u8; 3],
    #[serde(default = "default_background")]
    pub background: bool,
    pub image: String,
    pub repeat: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_mode: Option<RunMode>,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub debounce_ms: u32,
    #[serde(default = "default_title_size", skip_serializing_if = "is_default_title_size")]
    pub title_size: u8,
    #[serde(default = "default_icon_scale", skip_serializing_if = "is_default_icon_scale")]
    pub icon_scale: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginBinding>,
    pub steps: Vec<Step>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_display: Option<StateDisplay>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub long_steps: Vec<Step>,
    #[serde(skip)]
    pub display_kind: Option<usize>,
    #[serde(skip)]
    pub runtime_badge: Option<String>,
    #[serde(skip)]
    pub runtime_id: Option<String>,
}
fn is_zero_u32(value: &u32) -> bool { *value == 0 }
impl Default for Key {
    fn default() -> Self {
        Self {
            label: String::new(),
            color: [35, 44, 60],
            background: true,
            image: String::new(),
            repeat: 1,
            run_mode: None,
            debounce_ms: 0,
            title_size: 14,
            icon_scale: 100,
            plugin: None,
            steps: vec![],
            state_display: None,
            long_steps: vec![],
            display_kind: None,
            runtime_badge: None,
            runtime_id: None,
        }
    }
}
/// Overrides each individual face field only when it is explicitly provided.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StateFace {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StateDisplay {
    pub condition: crate::conditions::Condition,
    #[serde(default, skip_serializing_if = "is_false")]
    pub toggle: bool,
    pub on: StateFace,
    pub off: StateFace,
    #[serde(default = "default_state_poll_ms")]
    pub poll_ms: u64,
}
fn default_state_poll_ms() -> u64 { 1000 }
impl Default for StateDisplay {
    fn default() -> Self { Self { condition: Default::default(), toggle: false, on: Default::default(), off: Default::default(), poll_ms: default_state_poll_ms() } }
}
impl StateDisplay {
    pub fn validate(&self) -> Result<()> {
        if !self.toggle {
            self.condition.validate()?;
            anyhow::ensure!((250..=600_000).contains(&self.poll_ms), "Sondage de l'affichage : 250 à 600000 ms requis");
        }
        for face in [&self.on, &self.off] {
            if let Some(label) = &face.label { anyhow::ensure!(label.len() <= 128 && !label.chars().any(char::is_control), "Titre d'état invalide"); }
            if let Some(image) = &face.image { anyhow::ensure!(image.len() <= 1024 && !image.chars().any(char::is_control), "Icône d'état invalide"); }
        }
        Ok(())
    }
    pub fn apply(&self, key: &Key, active: bool) -> Key {
        let mut result = key.clone();
        let face = if active { &self.on } else { &self.off };
        if let Some(label) = &face.label { result.label.clone_from(label); }
        if let Some(image) = &face.image {
            if let Some(kind) = crate::audio::builtin_kind(image) {
                result.image.clear();
                result.display_kind = Some(kind);
            } else {
                result.image.clone_from(image);
            }
        }
        if let Some(color) = face.color { result.color = color; result.background = true; }
        result
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
/// Les pages gardent leur contenu lors d'un changement de périphérique.
pub fn extend_for_device(config: &mut Config, count: usize) {
    let count = count.clamp(1, 32);
    for profile in &mut config.profiles {
        if profile.keys.len() < count { profile.keys.resize_with(count, Key::default); }
        for page in &mut profile.pages {
            if page.keys.len() < count { page.keys.resize_with(count, Key::default); }
        }
    }
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
            color: Key::default().color,
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
            color: Key::default().color,
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
            bail!("Réglages du périphérique invalides");
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
                if !(1..=32).contains(&keys.len()) {
                    bail!("Chaque page doit contenir entre 1 et 32 touches");
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
/// Converts a legacy `FocusAudio.exe` launch (Codex module) into the equivalent native
/// application-volume block. Only exact, well-formed calls are recognised.
pub fn focus_audio_step(step: &Step) -> Option<Step> {
    let Step::Launch { program, args, .. } = step else { return None };
    if !program.trim().to_ascii_lowercase().ends_with("focusaudio.exe") { return None; }
    let number = |i: usize| args.get(i).and_then(|a| a.trim().parse::<u8>().ok());
    let (action, step, level) = match (args.first().map(|a| a.trim()), args.len()) {
        (Some("up"), 2) => ("up", number(1)?, 0),
        (Some("down"), 2) => ("down", number(1)?, 0),
        (Some("mute"), 1) => ("toggle_mute", 5, 0),
        (Some("set"), 2) => ("set", 5, number(1).filter(|v| *v <= 100)?),
        _ => return None,
    };
    if step == 0 || step > 100 { return None; }
    Some(Step::AppVolume { target: "foreground".into(), action: action.into(), step, level, include_children: true })
}
/// Replaces every legacy FocusAudio step, including inside conditions, repeats and toggles.
pub fn migrate_focus_audio(steps: &mut [Step]) -> usize {
    let mut count = 0;
    for step in steps.iter_mut() {
        if let Some(native) = focus_audio_step(step) { *step = native; count += 1; continue; }
        match step {
            Step::IfCondition { then_steps, else_steps, .. }
            | Step::IfWindow { then_steps, else_steps, .. }
            | Step::IfVariable { then_steps, else_steps, .. } => count += migrate_focus_audio(then_steps) + migrate_focus_audio(else_steps),
            Step::Repeat { steps, .. } => count += migrate_focus_audio(steps),
            Step::Toggle { a, b } => count += migrate_focus_audio(a) + migrate_focus_audio(b),
            _ => {}
        }
    }
    count
}
pub fn migrate_focus_audio_key(key: &mut Key) -> usize { migrate_focus_audio(&mut key.steps) + migrate_focus_audio(&mut key.long_steps) }
impl Config {
    pub fn migrate_focus_audio(&mut self) -> usize {
        self.profiles.iter_mut().map(|p| p.keys.iter_mut().chain(p.pages.iter_mut().flat_map(|g| g.keys.iter_mut())).map(migrate_focus_audio_key).sum::<usize>()).sum()
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
    anyhow::ensure!(key.debounce_ms <= 60_000, "Anti-rebond : 60 000 ms maximum");
    if let Some(display) = &key.state_display { display.validate()?; }
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
            Step::IfCondition { condition, .. } => condition.validate()?,
            Step::Notify { title, message } => {
                anyhow::ensure!(!title.trim().is_empty() && title.encode_utf16().count() <= 63 && !title.chars().any(char::is_control) && !message.trim().is_empty() && message.len() <= 2048, "Titre ou message de notification invalide");
            }
            Step::WaitUntil { condition, timeout_ms, poll_ms } => {
                condition.validate()?;
                anyhow::ensure!((1..=600_000).contains(timeout_ms) && *poll_ms >= 50 && *poll_ms <= *timeout_ms, "Délai (1 à 600000 ms) ou sondage (50 ms minimum, au plus le délai) invalide");
            }
            _ => {}
        }
        match s {
            Step::PluginAction { plugin, action, settings, label }
                if !valid_plugin_identifier(plugin) || !valid_plugin_identifier(action)
                    || !settings.is_object() || label.len() > 256 || label.chars().any(char::is_control) =>
                bail!("Action de plugin : identifiants, libellé ou réglages JSON invalides"),
            Step::MouseClick { button, x, y, relative_to, .. } if !["left", "right", "middle"].contains(&button.as_str()) || !valid_mouse_origin(relative_to) || x.is_some() != y.is_some() || x.is_some_and(|v| !mouse_coordinate_valid(v)) || y.is_some_and(|v| !mouse_coordinate_valid(v)) => bail!("Bouton, origine ou coordonnées du clic souris invalides"),
            Step::MouseMove { x, y, relative_to, .. } if !mouse_coordinate_valid(*x) || !mouse_coordinate_valid(*y) || !valid_mouse_origin(relative_to) => bail!("Position ou origine souris invalide"),
            Step::MouseScroll { amount, .. } if !(-100..=100).contains(amount) => bail!("Défilement souris hors limites"),
            Step::PlaySound { path, volume } if path.trim().is_empty() || path.len() > 1024 || path.contains('"') || path.chars().any(char::is_control) || !["wav", "mp3"].iter().any(|ext| std::path::Path::new(path).extension().is_some_and(|e| e.eq_ignore_ascii_case(ext))) || *volume > 100 => bail!("Son WAV/MP3 ou volume invalide"),
            Step::StopSounds => {},
            Step::Toggle { a, b } => { for steps in [a, b] { validate_key(&Key { steps: steps.clone(), ..Key::default() }, config)?; } },
            Step::UdpSend { host, port, text } if !valid_udp_host(host) || *port == 0 || text.len() > 65507 => bail!("Destination UDP ou message invalide"),
            Step::HttpRequest { method, url, body, content_type } => validate_http(method, url, body, content_type)?,
            Step::D6Brightness { mode, value } if !["set", "up", "down"].contains(&mode.as_str()) || *value > 100 => bail!("Mode ou luminosité invalide"),
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
            Step::KeyHold { keys, action, hold_ms } => {
                parse_hotkey(keys)?;
                anyhow::ensure!(["down", "up"].contains(&action.as_str()) && *hold_ms <= 3_600_000 && (action != "up" || *hold_ms == 0), "Maintien de touche : action ou durée invalide");
            }
            Step::Text { text } if text.len() > 100_000 => bail!("Texte trop long"),
            Step::Launch { program, args, .. } if program.trim().is_empty() || args.len() > 128 => {
                bail!("Application ou arguments invalides")
            }
            Step::Url { url }
                if !(url.starts_with("https://") || url.starts_with("http://") || (url.starts_with('{') && url.contains('}')))
                    || url.len() > 2048 || url.chars().any(char::is_control) =>
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
            Step::AppVolume { target, action, step, level, .. }
                if (target != "foreground" && (target.len() > 260 || !target.to_ascii_lowercase().ends_with(".exe") || target.contains(['/', '\\']) || target.chars().any(char::is_control) || target.trim() != target))
                    || !["up", "down", "set", "mute", "unmute", "toggle_mute"].contains(&action.as_str())
                    || !(1..=100).contains(step) || *level > 100 => bail!("Cible, action ou niveau de volume d’application invalide"),
            Step::WindowOp { target, value, op, x, y, w, h, monitor }
                if !valid_window_op(target, value, op, *x, *y, *w, *h, *monitor) => bail!("Fenêtre : cible, opération, dimensions ou écran invalide"),
            Step::Clipboard { op, value, name } if !["copy_text", "read_to_variable", "clear", "save", "restore"].contains(&op.as_str())
                || value.len() > 100_000 || value.contains('\0') || !valid_math_name(name) => bail!("Presse-papiers : opération, texte ou nom de variable invalide"),
            Step::DeviceVolume { device, action, step, level, .. }
                if device.trim().is_empty() || device.len() > 512 || device.chars().any(char::is_control)
                    || !["up", "down", "set", "mute", "unmute", "toggle_mute"].contains(&action.as_str())
                    || !(1..=100).contains(step) || *level > 100 => bail!("Périphérique, action ou niveau de volume invalide"),
            // Une touche peut être posée avant le choix des deux périphériques (réglés ensuite dans
            // l'inspecteur) ; seul un même périphérique choisi deux fois est refusé.
            Step::AudioSwitch { first, second, .. }
                if !first.is_empty() && first == second =>
            {
                bail!("Choisis deux périphériques audio différents")
            }
            Step::IfWindow { title, .. } if title.trim().is_empty() => {
                bail!("Titre de fenêtre vide")
            }
            Step::TextOp { name, op, value, find, .. } => {
                anyhow::ensure!(valid_math_name(name) && valid_text_op(op) && value.len() <= 4096 && find.len() <= 4096
                    && (op != "replace" || !find.is_empty()), "Bloc Texte : nom, opération ou texte de recherche invalide");
            }
            Step::Math { name, op, value, min, max, .. } => {
                anyhow::ensure!(valid_math_name(name) && value.len() <= 4096 && min.is_finite() && max.is_finite() && min <= max
                    && ["set", "add", "subtract", "multiply", "divide", "modulo", "min", "max", "random"].contains(&op.as_str()), "Bloc Calculer : nom, opération ou bornes invalides");
            }
            Step::SetVariable { name, value, .. } | Step::IfVariable { name, value, .. }
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
            Step::IfCondition { then_steps, else_steps, .. }
            | Step::IfWindow {
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
    let rest = if url.starts_with('{') && url.contains('}') {
        "{gabarit}"
    } else {
        url.strip_prefix("http://").ok_or_else(|| anyhow::anyhow!("Adresse HTTP requise"))?
    };
    let template_host = rest.starts_with('{') && rest.contains('}');
    let authority = rest.split('/').next().unwrap_or("");
    let (host, port) = authority.rsplit_once(':').unwrap_or((authority, "80"));
    anyhow::ensure!((valid_udp_host(host) || template_host) && (template_host || port.parse::<u16>().is_ok_and(|n| n != 0))
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
            Step::IfCondition { then_steps, else_steps, .. }
            | Step::IfWindow {
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
mod device_page_tests {
    use super::*;
    #[test]
    fn legacy_pages_survive_expansion_and_smaller_devices() {
        let mut c = Config::default();
        let original = serde_json::to_value(&c).unwrap();
        assert_eq!(c.keys().len(), 15);
        c.validate_structure().unwrap();
        extend_for_device(&mut c, 6);
        assert_eq!(serde_json::to_value(&c).unwrap(), original);
        c.keys_mut()[14].label = "Conservée".into();
        extend_for_device(&mut c, 32);
        assert_eq!(c.keys().len(), 32);
        assert_eq!(c.keys()[14].label, "Conservée");
        assert!(c.keys()[31].steps.is_empty());
        c.validate_structure().unwrap();
        let roundtrip: Config = serde_json::from_value(serde_json::to_value(&c).unwrap()).unwrap();
        assert_eq!(roundtrip.keys().len(), 32);
        c.keys_mut().push(Key::default());
        assert!(c.validate_structure().is_err());
        c.keys_mut().clear();
        assert!(c.validate_structure().is_err());
    }
}
#[cfg(test)]
mod window_op_tests {
    use super::*;
    #[test]
    fn defaults_roundtrip_validation_and_unknown_fields() {
        let step: Step = serde_json::from_str(r#"{"type":"window_op"}"#).unwrap();
        assert!(step.window_op_summary().unwrap().contains("Maximiser"));
        assert_eq!(step, serde_json::from_str(&serde_json::to_string(&step).unwrap()).unwrap());
        let c = Config::default();
        let valid = |s: Step| validate_key(&Key { steps: vec![s], ..Key::default() }, &c).is_ok();
        assert!(valid(step.clone()));
        for field in [r#""op":"wrong""#, r#""target":"exe","value":"bad/path.exe""#, r#""target":"title","value":"""#, r#""w":0"#, r#""monitor":17"#] {
            let s: Step = serde_json::from_str(&format!("{{\"type\":\"window_op\",{field}}}")).unwrap();
            assert!(!valid(s), "{field}");
        }
        assert!(serde_json::from_str::<Step>(r#"{"type":"window_op","surprise":1}"#).is_err());
    }
}
#[cfg(test)]
mod state_display_tests {
    use super::*;
    #[test]
    fn legacy_configs_and_state_faces_roundtrip_and_validate() {
        let legacy = serde_json::to_value(Key::default()).unwrap();
        assert!(legacy.get("state_display").is_none());
        let parsed: Key = serde_json::from_value(legacy).unwrap();
        assert!(parsed.state_display.is_none());
        let display: StateDisplay = serde_json::from_str(r#"{"condition":{"kind":"device_muted","input":true},"on":{"label":"Muet","color":[255,0,0]},"off":{"image":"builtin:microphone"}}"#).unwrap();
        assert_eq!(display.poll_ms, 1000);
        assert!(!display.toggle);
        assert!(serde_json::to_value(&display).unwrap().get("toggle").is_none());
        let key = Key { label: "Défaut".into(), state_display: Some(display.clone()), ..Key::default() };
        assert_eq!(serde_json::to_value(&key).unwrap()["state_display"]["on"]["label"], "Muet");
        let roundtrip: Key = serde_json::from_value(serde_json::to_value(&key).unwrap()).unwrap();
        assert_eq!(roundtrip.state_display, Some(display.clone()));
        assert_eq!(display.apply(&key, true).label, "Muet");
        assert_eq!(display.apply(&key, false).label, "Défaut");
        assert!(serde_json::from_str::<StateDisplay>(r#"{"unexpected":1}"#).is_err());
        assert!(serde_json::from_str::<StateFace>(r#"{"label":"x","unexpected":1}"#).is_err());
        for poll_ms in [0, 249, 600_001] {
            let bad = StateDisplay { poll_ms, ..display.clone() };
            assert!(validate_key(&Key { state_display: Some(bad), ..Key::default() }, &Config::default()).is_err());
        }
        assert!(validate_key(&Key { state_display: Some(StateDisplay { condition: crate::conditions::Condition::All { conditions: vec![] }, ..display }), ..Key::default() }, &Config::default()).is_err());
    }
    #[test]
    fn toggle_ignores_condition_and_poll_but_validates_faces() {
        let toggle = StateDisplay {
            toggle: true,
            condition: crate::conditions::Condition::All { conditions: vec![] },
            poll_ms: 0,
            on: StateFace { image: Some("on.png".into()), ..Default::default() },
            off: StateFace { image: Some("off.png".into()), ..Default::default() },
        };
        let key = Key { image: "normal.png".into(), state_display: Some(toggle.clone()), ..Key::default() };
        assert!(validate_key(&key, &Config::default()).is_ok());
        assert_eq!(toggle.apply(&key, false).image, "off.png");
        assert_eq!(toggle.apply(&key, true).image, "on.png");
        let builtin = StateDisplay { off: StateFace { image: Some("builtin:speakers".into()), ..Default::default() }, ..toggle.clone() };
        let shown = builtin.apply(&key, false);
        assert!(shown.image.is_empty());
        assert_eq!(shown.display_kind, crate::audio::builtin_kind("builtin:speakers"));
        let value = serde_json::to_value(&toggle).unwrap();
        assert_eq!(value["toggle"], true);
        assert_eq!(serde_json::from_value::<StateDisplay>(value).unwrap(), toggle);
        let bad = StateDisplay { off: StateFace { label: Some("x".repeat(129)), ..Default::default() }, ..toggle };
        assert!(bad.validate().is_err());
    }
}
#[cfg(test)]
mod key_hold_tests {
    use super::*;
    #[test]
    fn defaults_roundtrip_unknown_fields_and_validation() {
        let s: Step = serde_json::from_str(r#"{"type":"key_hold"}"#).unwrap();
        assert_eq!(s, Step::KeyHold { keys: "CTRL".into(), action: "down".into(), hold_ms: 0 });
        assert!(s.key_hold_summary().unwrap().contains("fin de la macro"));
        assert_eq!(serde_json::from_str::<Step>(&serde_json::to_string(&s).unwrap()).unwrap(), s);
        assert!(serde_json::from_str::<Step>(r#"{"type":"key_hold","unexpected":1}"#).is_err());
        for bad in [Step::KeyHold { keys: "?".into(), action: "down".into(), hold_ms: 0 }, Step::KeyHold { keys: "A".into(), action: "other".into(), hold_ms: 0 }, Step::KeyHold { keys: "A".into(), action: "up".into(), hold_ms: 1 }, Step::KeyHold { keys: "A".into(), action: "down".into(), hold_ms: 3_600_001 }] {
            assert!(validate_key(&Key { steps: vec![bad], ..Key::default() }, &Config::default()).is_err());
        }
        assert!(validate_key(&Key { steps: vec![s], ..Key::default() }, &Config::default()).is_ok());
        assert!(crate::automation::export(&Key { steps: vec![Step::KeyHold { keys: "A".into(), action: "up".into(), hold_ms: 0 }], ..Key::default() }).is_err());
    }
}

#[cfg(test)]
mod math_step_tests {
    use super::*;
    #[test]
    fn defaults_roundtrip_unknown_fields_and_validation() {
        let step: Step = serde_json::from_str(r#"{"type":"math"}"#).unwrap();
        assert_eq!(step, serde_json::from_str(&serde_json::to_string(&step).unwrap()).unwrap());
        assert_eq!(step.math_summary().as_deref(), Some("Ajouter 1 à compteur"));
        assert!(serde_json::from_str::<Step>(r#"{"type":"math","unexpected":1}"#).is_err());
        for op in ["bad", "divide"] {
            let bad = Step::Math { name: "x".into(), op: op.into(), value: "1".into(), min: 2.0, max: 1.0, persistent: false, };
            assert!(validate_key(&Key { steps: vec![bad], ..Key::default() }, &Config::default()).is_err());
        }
        let bad = Step::Math { name: "bad name".into(), op: "add".into(), value: "1".into(), min: 0.0, max: 1.0, persistent: false, };
        assert!(validate_key(&Key { steps: vec![bad], ..Key::default() }, &Config::default()).is_err());
    }
}
#[cfg(test)]
mod launch_option_tests {
    use super::*;
    #[test]
    fn launch_option_defaults_roundtrip_and_rejects_unknown() {
        let step: Step = serde_json::from_str(r#"{"type":"launch","program":"notepad.exe","args":[]}"#).unwrap();
        assert!(matches!(step, Step::Launch { focus_if_running: false, .. }));
        assert_eq!(step, serde_json::from_str(&serde_json::to_string(&step).unwrap()).unwrap());
        assert!(serde_json::from_str::<Step>(r#"{"type":"launch","program":"notepad.exe","args":[],"unknown":true}"#).is_err());
        assert!(serde_json::from_str::<Step>(r#"{"type":"launch","program":"notepad.exe","args":[],"focus_if_running":"yes"}"#).is_err());
        assert!(validate_key(&Key { steps: vec![Step::Launch { program: "".into(), args: vec![], focus_if_running: true }], ..Key::default() }, &Config::default()).is_err());
    }
}

#[cfg(test)]
mod condition_step_tests {
    use super::*;
    #[test]
    fn defaults_roundtrip_validation_and_summary() {
        let s: Step = serde_json::from_str(r#"{"type":"if_condition"}"#).unwrap();
        assert!(matches!(&s, Step::IfCondition { then_steps, else_steps, .. } if then_steps.is_empty() && else_steps.is_empty()));
        assert_eq!(s, serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap());
        assert!(validate_key(&Key { steps: vec![s.clone()], ..Key::default() }, &Config::default()).is_ok());
        assert!(matches!(s, Step::IfCondition { condition, .. } if condition.summary().contains("premier plan")));
        assert!(serde_json::from_str::<Step>(r#"{"type":"if_condition","unknown":3}"#).is_err());
        let bad = Step::IfCondition { condition: crate::conditions::Condition::All { conditions: vec![] }, then_steps: vec![], else_steps: vec![] };
        assert!(validate_key(&Key { steps: vec![bad], ..Key::default() }, &Config::default()).is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persisted_variable_flags_default_roundtrip_and_reject_unknown() {
        for kind in ["set_variable", "math", "text_op"] {
            let plain: Step = serde_json::from_value(serde_json::json!({"type":kind})).unwrap();
            assert_eq!(serde_json::to_value(&plain).unwrap()["persistent"], false);
            let saved: Step = serde_json::from_value(serde_json::json!({"type":kind,"persistent":true})).unwrap();
            assert_eq!(serde_json::to_value(&saved).unwrap()["persistent"], true);
            assert_eq!(saved, serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap());
            assert!(serde_json::from_value::<Step>(serde_json::json!({"type":kind,"persistent":"oui"})).is_err());
            assert!(serde_json::from_value::<Step>(serde_json::json!({"type":kind,"extra":false})).is_err());
            assert!(validate_key(&Key { steps: vec![saved], ..Key::default() }, &Config::default()).is_ok());
        }
    }
    #[test]
    fn text_op_serde_validation_and_summary() {
        let step: Step = serde_json::from_str(r#"{"type":"text_op"}"#).unwrap();
        assert_eq!(step, Step::TextOp { name: "texte".into(), op: "upper".into(), value: String::new(), find: String::new(), persistent: false, });
        let round: Step = serde_json::from_value(serde_json::to_value(&step).unwrap()).unwrap();
        assert_eq!(round, step);
        assert_eq!(step.text_op_summary().unwrap(), "Mettre texte en majuscules");
        assert!(serde_json::from_str::<Step>(r#"{"type":"text_op","surprise":1}"#).is_err());
        for (name, op, value, find) in [("", "upper", "", ""), ("avec espace", "set", "", ""), ("ok", "invalid", "", ""), ("ok", "replace", "x", "")] {
            let bad = Step::TextOp { name: name.into(), op: op.into(), value: value.into(), find: find.into(), persistent: false, };
            assert!(validate_key(&Key { steps: vec![bad], ..Key::default() }, &Config::default()).is_err());
        }
        assert!(validate_key(&Key { steps: vec![step], ..Key::default() }, &Config::default()).is_ok());
    }
    #[test]
    fn focus_audio_launches_become_native_app_volume() {
        let legacy = |args: &[&str]| Step::Launch { program: "C:\\DeckForge\\assets\\automations-codex\\module\\FocusAudio.exe".into(), args: args.iter().map(|a| a.to_string()).collect(), focus_if_running: false };
        let app = |action: &str, step: u8, level: u8| Step::AppVolume { target: "foreground".into(), action: action.into(), step, level, include_children: true };
        assert_eq!(focus_audio_step(&legacy(&["up", "5"])), Some(app("up", 5, 0)));
        assert_eq!(focus_audio_step(&legacy(&["down", "5"])), Some(app("down", 5, 0)));
        assert_eq!(focus_audio_step(&legacy(&["mute"])), Some(app("toggle_mute", 5, 0)));
        assert_eq!(focus_audio_step(&legacy(&["set", "50"])), Some(app("set", 5, 50)));
        for bad in [&["set", "150"][..], &["up"], &["mute", "1"], &["louder", "5"], &["up", "0"]] { assert_eq!(focus_audio_step(&legacy(bad)), None, "{bad:?}"); }
        assert_eq!(focus_audio_step(&Step::Launch { program: "notepad.exe".into(), args: vec!["up".into(), "5".into()], focus_if_running: false }), None);
        let mut key = Key { label: "Audio".into(), steps: vec![Step::Repeat { count: 2, steps: vec![legacy(&["up", "5"])] }], long_steps: vec![legacy(&["mute"])], ..Key::default() };
        assert_eq!(migrate_focus_audio_key(&mut key), 2);
        assert_eq!(key.label, "Audio");
        assert_eq!(key.long_steps, vec![app("toggle_mute", 5, 0)]);
        assert_eq!(migrate_focus_audio_key(&mut key), 0);
    }
    #[test]
    fn notify_defaults_roundtrip_and_validation() {
        let s: Step = serde_json::from_str(r#"{"type":"notify","message":"Bonjour {nom}"}"#).unwrap();
        assert_eq!(s, Step::Notify { title: "NovyDeck".into(), message: "Bonjour {nom}".into() });
        assert_eq!(serde_json::from_str::<Step>(&serde_json::to_string(&s).unwrap()).unwrap(), s);
        assert!(serde_json::from_str::<Step>(r#"{"type":"notify","message":"ok","unknown":1}"#).is_err());
        assert!(s.name().contains("Notification"));
        assert!(validate_key(&Key { steps: vec![Step::Notify { title: "".into(), message: "ok".into() }], ..Key::default() }, &Config::default()).is_err());
    }
    #[test]
    fn wait_until_defaults_roundtrip_and_validation() {
        let s: Step = serde_json::from_str(r#"{"type":"wait_until"}"#).unwrap();
        assert!(matches!(&s, Step::WaitUntil { timeout_ms: 5000, poll_ms: 200, .. }));
        assert_eq!(serde_json::from_str::<Step>(&serde_json::to_string(&s).unwrap()).unwrap(), s);
        assert!(serde_json::from_str::<Step>(r#"{"type":"wait_until","surprise":1}"#).is_err());
        assert!(s.name().contains("Attendre"));
        for (timeout_ms, poll_ms) in [(600_001, 200), (5000, 49), (100, 200)] {
            let bad = Step::WaitUntil { condition: crate::conditions::Condition::default(), timeout_ms, poll_ms };
            assert!(validate_key(&Key { steps: vec![bad], ..Key::default() }, &Config::default()).is_err());
        }
    }
    #[test]
    fn device_volume_defaults_roundtrip_and_validation() {
        let s: Step = serde_json::from_str(r#"{"type":"device_volume"}"#).unwrap();
        assert_eq!(s, Step::DeviceVolume { input: false, device: "default".into(), action: "up".into(), step: 5, level: 50 });
        assert_eq!(serde_json::from_value::<Step>(serde_json::to_value(&s).unwrap()).unwrap(), s);
        assert!(s.device_volume_summary().unwrap().contains("sortie audio"));
        assert!(serde_json::from_str::<Step>(r#"{"type":"device_volume","surprise":1}"#).is_err());
        for bad in [Step::DeviceVolume { input: false, device: "default".into(), action: "up".into(), step: 0, level: 50 }, Step::DeviceVolume { input: false, device: "".into(), action: "up".into(), step: 5, level: 50 }] {
            assert!(validate_key(&Key { steps: vec![bad], ..Key::default() }, &Config::default()).is_err());
        }
    }
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
        for step in [Step::MouseClick { button: "left".into(), double: true, x: Some(42), y: Some(-5), relative_to: "window".into() }, Step::MouseMove { x: 14, y: -5, relative: false, relative_to: "cursor".into() }, Step::MouseScroll { amount: -3, horizontal: true }] {
            let json = serde_json::to_string(&step).unwrap();
            assert_eq!(serde_json::from_str::<Step>(&json).unwrap(), step);
            validate_key(&Key { steps: vec![step], ..Key::default() }, &c).unwrap();
        }
        for invalid in [Step::MouseClick { button: "other".into(), double: false, x: None, y: None, relative_to: "screen".into() }, Step::MouseClick { button: "left".into(), double: false, x: Some(1), y: None, relative_to: "window".into() }, Step::MouseScroll { amount: 101, horizontal: true }, Step::MouseMove { x: 99999, y: 0, relative: false, relative_to: "screen".into() }, Step::MouseMove { x: 0, y: 0, relative: false, relative_to: "invalid".into() }] {
            assert!(validate_key(&Key { steps: vec![invalid], ..Key::default() }, &c).is_err());
        }
        let legacy: Step = serde_json::from_str(r#"{"type":"mouse_move","x":5,"y":-2,"relative":true}"#).unwrap();
        assert!(matches!(legacy, Step::MouseMove { relative: true, relative_to, .. } if relative_to == "screen"));
        assert!(matches!(serde_json::from_str::<Step>(r#"{"type":"mouse_click"}"#).unwrap(), Step::MouseClick { x: None, y: None, .. }));
        assert!(serde_json::from_str::<Step>(r#"{"type":"mouse_scroll","extra":1}"#).is_err());
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

#[cfg(test)]
mod interpolation_template_tests {
    use super::*;
    #[test]
    fn templates_survive_serialization_and_validate_without_execution() {
        let steps: Vec<Step> = serde_json::from_value(serde_json::json!([
            {"type":"text","text":"Bonjour {nom} {date}"},
            {"type":"url","url":"{adresse}"},
            {"type":"http_request","method":"POST","url":"{adresse}","body":"{presse_papiers}","content_type":"text/plain"},
            {"type":"udp_send","host":"localhost","port":9999,"text":"{heure}"},
            {"type":"launch","program":"notepad.exe","args":["{horodatage}"]}
        ])).unwrap();
        validate_key(&Key { steps: steps.clone(), ..Key::default() }, &Config::default()).unwrap();
        let encoded = serde_json::to_string(&steps).unwrap();
        assert!(encoded.contains("{presse_papiers}"));
        assert_eq!(steps, serde_json::from_str::<Vec<Step>>(&encoded).unwrap());
        assert!(serde_json::from_str::<Step>(r#"{"type":"text","text":"ok","extra":true}"#).is_err());
        assert!(validate_http("POST", "https://example.org", "", "text/plain").is_err());
        assert!(validate_http("POST", "http://", "", "text/plain").is_err());
    }
}

#[cfg(test)]
mod clipboard_tests {
    use super::*;
    #[test]
    fn defaults_roundtrip_validation_and_unknown_field() {
        let step: Step = serde_json::from_str(r#"{"type":"clipboard"}"#).unwrap();
        assert_eq!(step, Step::Clipboard { op: "copy_text".into(), value: String::new(), name: "texte".into() });
        assert_eq!(serde_json::from_str::<Step>(&serde_json::to_string(&step).unwrap()).unwrap(), step);
        assert!(serde_json::from_str::<Step>(r#"{"type":"clipboard","unexpected":1}"#).is_err());
        assert!(step.clipboard_summary().unwrap().contains("Copier"));
        let config = Config::default();
        for (op, value, name) in [("bad", "", "texte"), ("copy_text", "a\0b", "texte"), ("read_to_variable", "", "nom incorrect")] {
            let key = Key { steps: vec![Step::Clipboard { op: op.into(), value: value.into(), name: name.into() }], ..Key::default() };
            assert!(validate_key(&key, &config).is_err());
        }
    }
}

#[cfg(test)]
mod execution_options_tests {
    use super::*;
    #[test]
    fn options_json_legacy_roundtrip_and_validation() {
        let legacy = serde_json::to_value(Key::default()).unwrap();
        assert!(legacy.get("run_mode").is_none());
        assert!(legacy.get("debounce_ms").is_none());
        let loaded: Key = serde_json::from_value(legacy).unwrap();
        assert_eq!(loaded.run_mode, None);
        assert_eq!(loaded.debounce_ms, 0);
        let mut configured = loaded;
        configured.run_mode = Some(RunMode::Queue);
        configured.debounce_ms = 250;
        let value = serde_json::to_value(&configured).unwrap();
        assert_eq!(value["run_mode"], "queue");
        let copy: Key = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(copy.run_mode, Some(RunMode::Queue));
        assert_eq!(copy.debounce_ms, 250);
        assert!(validate_key(&copy, &Config::default()).is_ok());
        assert!(serde_json::from_value::<Key>({ let mut v=value.clone(); v["run_mode"]="invalid".into(); v }).is_err());
        let mut bad = configured;
        bad.debounce_ms = 60_001;
        assert!(validate_key(&bad, &Config::default()).is_err());
    }
}
