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
    add("Système", "Clic souris", "Clique sur le bouton gauche de la souris.", [73, 160, 217], vec![Step::MouseClick { button: "left".into(), double: false, x: None, y: None, relative_to: "screen".into() }]);
    add("Système", "Déplacer la souris", "Déplace le curseur relativement à sa position.", [73, 160, 217], vec![Step::MouseMove { x: 10, y: 0, relative: false, relative_to: "cursor".into() }]);
    add("Système", "Défiler", "Fait défiler la fenêtre active d'un cran.", [73, 160, 217], vec![Step::MouseScroll { amount: 1, horizontal: false }]);
    add("Système", "Fermer la fenêtre", "Demande à Windows de fermer la fenêtre active, sauf DeckForge.", [73, 160, 217], vec![Step::CloseWindow]);
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
    for (label, op, monitor) in [("Fenêtre sur l'écran 2", "move_to_monitor", 2), ("Toujours au premier plan", "toggle_always_on_top", 2), ("Maximiser", "maximize", 2)] {
        add("Fenêtres", label, "Opération sur la fenêtre active, sauf DeckForge ; écran 1 = principal.", [73, 160, 217], vec![Step::WindowOp {
            target: "foreground".into(), value: String::new(), op: op.into(), x: 0, y: 0, w: 800, h: 600, monitor,
        }]);
    }
    add("Clavier", "Maintenir Ctrl 500 ms", "Maintient Ctrl puis le relâche après une demi-seconde ; Arrêter relâche immédiatement.", [73, 160, 217], vec![Step::KeyHold { keys: "CTRL".into(), action: "down".into(), hold_ms: 500 }]);
    add("Texte", "Coller sans mise en forme", "Lit le texte Unicode, le recopie sans formatage puis colle dans la fenêtre active.", [108, 135, 236], vec![
        Step::Clipboard { op: "read_to_variable".into(), value: String::new(), name: "texte_colle".into() },
        Step::Clipboard { op: "copy_text".into(), value: "{texte_colle}".into(), name: "texte".into() },
        Step::Hotkey { keys: "CTRL+V".into() },
    ]);
    add("Variables", "Mettre en majuscules", "Met en majuscules la variable texte pendant cette macro.", [108, 135, 236], vec![Step::TextOp {
        name: "texte".into(), op: "upper".into(), value: String::new(), find: String::new(),
   persistent: false, }]);
    for (label, op, value) in [("Compteur +1", "add", "1"), ("Compteur −1", "subtract", "1"), ("Compteur à 0", "set", "0")] {
        add("Variables", label, "Calcule la valeur numérique du compteur pendant cette macro.", [108, 135, 236], vec![Step::Math { name: "compteur".into(), op: op.into(), value: value.into(), min: 0.0, max: 100.0, persistent: false, }]);
    }
    for (label, action, level) in [("App +5 %", "up", 0), ("App −5 %", "down", 0), ("App muette", "toggle_mute", 0), ("App à 50 %", "set", 50)] {
        add("Audio", label, "Volume de l’application active, sans modifier le volume général.", [76, 166, 137], vec![Step::AppVolume {
            target: "foreground".into(), action: action.into(), step: 5, level, include_children: true,
        }]);
    }
    for (label, input, action) in [("Micro muet / rétabli", true, "toggle_mute"), ("Micro +5 %", true, "up"), ("Micro −5 %", true, "down"), ("Sortie +5 %", false, "up"), ("Sortie −5 %", false, "down")] {
        add("Audio", label, "Volume du périphérique audio par défaut, différent du volume d'une application.", [76, 166, 137], vec![Step::DeviceVolume {
            input, device: "default".into(), action: action.into(), step: 5, level: 50,
        }]);
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
    add("Automatisations", "Condition sur une fenêtre", "Si une fenêtre contenant ce titre est ouverte, exécuter les blocs ALORS, sinon les blocs SINON. Personnalise dans l’Atelier.", [158,116,224], vec![Step::IfWindow{title:"Chrome".into(),then_steps:vec![Step::Delay{ms:250}],else_steps:vec![Step::Delay{ms:500}]}]);
    add("Automatisations", "Boucle de blocs", "Répéter une séquence de blocs. Cet exemple attend deux fois 250 ms, sans action sur ton PC.", [224,161,84], vec![Step::Repeat{count:2,steps:vec![Step::Delay{ms:250}]}]);
    add(
        "Automatisations",
        "Variable et condition",
        "Définir une variable locale à l’exécution, puis choisir une branche selon sa valeur.",
        [158, 116, 224],
        vec![
            Step::SetVariable {
                name: "mode".into(),
                value: "travail".into(),
                    persistent: false,
                },
            Step::IfVariable {
                name: "mode".into(),
                value: "travail".into(),
                then_steps: vec![Step::Delay { ms: 250 }],
                else_steps: vec![],
            },
        ],
    );
    for (label, keys, description) in [
        ("Discord : Micro", "CTRL+SHIFT+F13", "Définis le même raccourci dans Discord > Paramètres > Raccourcis clavier > Couper le micro. État non synchronisé."),
        ("Discord : Casque", "CTRL+SHIFT+F14", "Définis le même raccourci dans Discord > Paramètres > Raccourcis clavier > Couper le son. État non synchronisé."),
        ("Navigateur : Retour", "BROWSER_BACK", "Revenir à la page précédente (touche navigateur Windows)."),
        ("Navigateur : Suivant", "BROWSER_FORWARD", "Aller à la page suivante (touche navigateur Windows)."),
        ("Navigateur : Actualiser", "BROWSER_REFRESH", "Actualiser la page (touche navigateur Windows)."),
        ("Fermer la fenêtre", "ALT+F4", "Fermer la fenêtre active ; attention aux modifications non enregistrées."),
        ("Afficher le bureau", "WIN+D", "Afficher ou masquer le bureau Windows."),
        ("Reconnaissance vocale", "WIN+H", "Ouvrir la saisie vocale Windows."),
        ("Changer d’écran", "WIN+P", "Ouvrir les options de projection Windows."),
    ] {
        let category = if label.starts_with("Discord") { "Streaming" } else { "Système" };
        add(category, label, description, [108, 154, 218], vec![Step::Hotkey { keys: keys.into() }]);
    }
    add("Système", "Basculer 1 / 2", "Alterner deux actions à chaque appui sur cette touche.", [73, 160, 217], vec![Step::Toggle { a: vec![Step::Delay { ms: 100 }], b: vec![Step::Delay { ms: 200 }] }]);
    add("Streaming", "Envoyer en UDP", "Envoie un message à un service local (vMix).", [76, 166, 137], vec![Step::UdpSend { host: "127.0.0.1".into(), port: 8099, text: "message".into() }]);
    add("Streaming", "Requête HTTP", "Appelle une API HTTP locale (sans HTTPS).", [76, 166, 137], vec![Step::HttpRequest { method: "GET".into(), url: "http://127.0.0.1:8088/api".into(), body: String::new(), content_type: "text/plain".into() }]);
    add("Streaming", "Jouer un son", "Lit un fichier WAV ou MP3 sans interrompre la macro (8 sons maximum).", [76, 166, 137], vec![Step::PlaySound { path: "choisir.wav".into(), volume: 100 }]);
    add("Streaming", "Arrêter les sons", "Arrête les sons lancés par DeckForge.", [76, 166, 137], vec![Step::StopSounds]);
    add("Automatisations", "Si… sinon", "Choisit ALORS ou SINON suivant une condition Windows en lecture seule.", [158,116,224], vec![Step::IfCondition { condition: crate::conditions::Condition::default(), then_steps: vec![], else_steps: vec![] }]);
    add("Automatisations", "Attendre que…", "Attend une condition en lecture seule pendant 5 secondes au plus.", [158,116,224], vec![Step::WaitUntil { condition: crate::conditions::Condition::default(), timeout_ms: 5000, poll_ms: 200 }]);
    add("Automatisations", "Notification", "Affiche un message Windows sans voler le focus.", [158,116,224], vec![Step::Notify { title: "DeckForge".into(), message: "Bonjour {nom}".into() }]);
    drop(add);
    for (label, description, condition, true_label, false_label, color) in [
        ("Micro coupé = icône rouge", "Le microphone muet affiche un état rouge, même après un changement externe.", crate::conditions::Condition::DeviceMuted { input: true, device: "default".into() }, "Micro coupé", "Micro actif", [215, 58, 69]),
        ("Application muette", "Lit l'état muet de l'application au premier plan.", crate::conditions::Condition::AppMuted { exe: "foreground".into() }, "App muette", "App audible", [215, 58, 69]),
        ("Casque branché", "Détecte un périphérique de sortie actif dont le nom contient Casque.", crate::conditions::Condition::AudioDevicePresent { name_contains: "Casque".into(), input: false }, "Casque branché", "Casque absent", [67, 158, 124]),
    ] {
        items.push(Preset { category: "Audio", description,
            key: Key { label: label.into(), color: [76, 166, 137],
                steps: vec![Step::Delay { ms: 1 }],
                state_display: Some(crate::model::StateDisplay { condition,
                    on: crate::model::StateFace { label: Some(true_label.into()), image: Some("builtin:microphone".into()), color: Some(color) },
                    off: crate::model::StateFace { label: Some(false_label.into()), ..Default::default() },
                    ..Default::default() }), ..Key::default() } });
    }
    items
}

pub fn action(kind: usize, profile: &str) -> Key {
    let step = if kind >= 13 {
        crate::automation::step(kind)
    } else {
        match kind {
            0 => Step::Hotkey {
                keys: "CTRL+C".into(),
            },
            1 => Step::Text {
                text: "Mon texte".into(),
            },
            2 => Step::Launch {
                program: "explorer.exe".into(),
                args: vec![],
                focus_if_running: false,
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
        }
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
    pub close_to_tray: bool,
    pub start_with_windows: bool,
    pub triggers: Vec<crate::desktop::Trigger>,
    #[serde(default)]
    pub workshop_advanced: bool,
    #[serde(default = "default_chat_mode")]
    pub workshop_chat_mode: String,
    #[serde(default)]
    pub allow_connected_ai: bool,
    /// Style choisi (navy, graphite, clair, sable) ; vide = déduit de `dark` (anciens fichiers).
    #[serde(default)]
    pub scheme: String,
    /// Couleur d'accent (bleu, violet, emeraude, orange, rose).
    #[serde(default)]
    pub accent: String,
    /// IA locale facultative de l'assistant de l'atelier (désactivée par défaut).
    #[serde(default)]
    pub local_ai: LocalAi,
    /// Coupe l'intro et les fondus (l'indicateur de chargement reste).
    #[serde(default)]
    pub reduce_motion: bool,
}
fn default_chat_mode() -> String { "auto".into() }

/// Réglages de l'IA locale : `ollama` (serveur installé à part) ou `integre` (moteur livré).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct LocalAi {
    pub enabled: bool,
    pub backend: String,
    pub endpoint: String,
    pub model: String,
}
impl Default for LocalAi {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: "ollama".into(),
            endpoint: "http://127.0.0.1:11434".into(),
            model: "qwen2.5:1.5b".into(),
        }
    }
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            dark: true,
            library: Library::default(),
            close_to_tray: true,
            start_with_windows: false,
            triggers: vec![],
            workshop_advanced: false,
            workshop_chat_mode: default_chat_mode(),
            allow_connected_ai: false,
            scheme: String::new(),
            accent: String::new(),
            local_ai: LocalAi::default(),
            reduce_motion: false,
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
            p.library.schema == 1 && p.library.binds.len() <= 128 && p.triggers.len() <= 256 && p.triggers.iter().all(|t| t.validate().is_ok()),
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
mod tests {    #[test]
    fn connected_ai_is_opt_in_and_roundtrips_preferences() {
        let mut value = serde_json::to_value(Preferences::default()).unwrap();
        value.as_object_mut().unwrap().remove("allow_connected_ai");
        let legacy: Preferences = serde_json::from_value(value).unwrap();
        assert!(!legacy.allow_connected_ai);
        let mut enabled = legacy;
        enabled.allow_connected_ai = true;
        let loaded: Preferences = serde_json::from_str(&serde_json::to_string(&enabled).unwrap()).unwrap();
        assert!(loaded.allow_connected_ai);
    }
    use super::*;
    #[test]
    fn every_preset_and_action_is_executable_with_valid_configuration() {
        let c = Config::default();
        assert_eq!(presets().len(), 82);
        for p in presets() {
            validate_key(&p.key, &c).unwrap();
            assert!(!p.key.steps.is_empty());
        }
        for i in 0..12 {
            validate_key(&action(i, "Bureau"), &c).unwrap();
        }
    }
    #[test]
    fn native_browser_and_discord_presets_are_serializable_and_valid() {
        let presets = presets();
        for label in ["Discord : Micro", "Discord : Casque", "Navigateur : Retour", "Navigateur : Suivant", "Navigateur : Actualiser"] {
            let p = presets.iter().find(|p| p.key.label == label).unwrap();
            validate_key(&p.key, &Config::default()).unwrap();
            let json = serde_json::to_string(&p.key).unwrap();
            assert_eq!(serde_json::from_str::<Key>(&json).unwrap().steps, p.key.steps);
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
    fn workshop_mode_is_backward_compatible_and_does_not_change_the_key() {
        let old = serde_json::to_value(Preferences::default()).unwrap();
        let mut old = old.as_object().unwrap().clone(); old.remove("workshop_advanced");
        let mut prefs: Preferences = serde_json::from_value(old.into()).unwrap();
        assert!(!prefs.workshop_advanced);
        let key = action(4, "Bureau");
        prefs.workshop_advanced = true;
        let saved = serde_json::to_string(&prefs).unwrap();
        let loaded: Preferences = serde_json::from_str(&saved).unwrap();
        assert!(loaded.workshop_advanced);
        assert_eq!(key.steps, action(4, "Bureau").steps);
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

#[cfg(test)]
mod clipboard_preset_tests {
    use super::*;
    #[test]
    fn plain_paste_reads_copies_then_pastes() {
        let preset = presets().into_iter().find(|p| p.key.label == "Coller sans mise en forme").unwrap();
        assert!(matches!(&preset.key.steps[..], [Step::Clipboard { op: read, name, .. }, Step::Clipboard { op: copy, value, .. }, Step::Hotkey { keys }] if read == "read_to_variable" && name == "texte_colle" && copy == "copy_text" && value == "{texte_colle}" && keys == "CTRL+V"));
        validate_key(&preset.key, &Config::default()).unwrap();
    }
}
