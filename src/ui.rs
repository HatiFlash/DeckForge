use crate::{
    catalog::{self, Library, Preferences},
    control::{Decision, InputGate, LongPress},
    device::{self, Command},
    engine::Engine,
    model::{validate_key, Config, Key, Profile, Step},
    store,
    theme::{self, *},
    Event,
};
use eframe::egui::{self, Color32, RichText, Vec2};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{
        atomic::Ordering,
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};

#[path = "ui_about.rs"]
mod about;

#[path = "ui_key_preview.rs"]
mod key_preview;

#[path = "ui_appearance.rs"]
mod appearance;

#[path = "ui_plugin_store.rs"]
mod plugin_store_ui;

#[derive(Default,Clone)]
struct PluginFace { title:String, image_bytes:Option<Vec<u8>>, state:u8, flash:Option<(bool,Instant)>, version:u64 }
#[cfg(test)]
#[test]
fn plugin_image_formats_and_limits() {
    assert_eq!(decode_plugin_image("").unwrap(),None);
    assert_eq!(decode_plugin_image("<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><rect width='16' height='16'/></svg>").unwrap().unwrap()[0],b'<');
    assert!(decode_plugin_image("data:image/png;base64,????").is_err());
    assert!(decode_plugin_image("data:text/html;base64,PHA+").is_err());
    assert!(decode_plugin_image(&"x".repeat(2_000_001)).is_err());
}
fn decode_plugin_image(value:&str)->anyhow::Result<Option<Vec<u8>>> {
    use base64::Engine as _;
    if value.is_empty() {return Ok(None);}
    let bytes=if let Some((mime,payload))=value.split_once(",") {
        anyhow::ensure!(matches!(mime,"data:image/png;base64"|"data:image/jpeg;base64"|"data:image/svg+xml;base64"),"Format d'image non pris en charge");
        anyhow::ensure!(payload.len()<=2_000_000,"Image trop volumineuse");
        base64::engine::general_purpose::STANDARD.decode(payload)?
    }else{
        anyhow::ensure!(value.trim_start().starts_with("<svg") && value.len()<=2_000_000,"SVG invalide");
        value.as_bytes().to_vec()
    };
    anyhow::ensure!(bytes.len()<=1_500_000,"Image trop volumineuse");
    crate::assets::decode_image(&bytes,512)?;
    Ok(Some(bytes))
}
type DialogEffect = Box<dyn FnOnce(&mut App) + Send>;
type StepTarget = (usize, String, usize, Vec<usize>);
fn choose_step_file(tx: Sender<Option<DialogEffect>>, target: StepTarget, executable: bool) {
    std::thread::spawn(move || {
        let path = rfd::FileDialog::new().add_filter(if executable {"AutoHotkey"} else {"Application"},
            if executable { &["exe"][..] } else { &["exe", "com"][..] }).pick_file();
        let _ = tx.send(path.map(|path| Box::new(move |app: &mut App| {
            let (profile,page,key,step_path)=target;
            if let Some(step)=app.config.profiles.get_mut(profile)
                .and_then(|p|p.keys_at_mut(&page)).and_then(|keys|keys.get_mut(key))
                .and_then(|key|extras::step_mut(&mut key.steps,&step_path)) {
                match step {
                    Step::Launch {program,..} if !executable => *program=path.to_string_lossy().into_owned(),
                    Step::AutoHotkey {executable: file,..} if executable => *file=path.to_string_lossy().into_owned(),
                    _ => return,
                }
                app.edited();
            }
        }) as DialogEffect));
    });
}

fn choose_profile(config: &Config, exe: Option<&str>, base: usize) -> usize {
    if !config.auto_profile { return base; }
    exe.and_then(|exe| config.profiles.iter().position(|profile|
        profile.apps.iter().any(|name| !name.trim().is_empty() && name.trim().eq_ignore_ascii_case(exe))))
        .unwrap_or(base)
}
struct SleepState { last: Instant, sleeping: bool }
impl SleepState {
    fn new(now: Instant) -> Self { Self { last: now, sleeping: false } }
    fn activity(&mut self, now: Instant) { self.last = now; self.sleeping = false; }
    fn due(&mut self, now: Instant, minutes: u16) -> bool {
        if minutes == 0 || self.sleeping || now.saturating_duration_since(self.last) < Duration::from_secs(u64::from(minutes) * 60) { return false; }
        self.sleeping = true; true
    }
    fn press(&mut self, now: Instant) -> bool {
        let wake = self.sleeping;
        self.activity(now);
        wake
    }
}

pub struct App {
    config: Config,
    applied: Config,
    pending: Option<(u64, Config)>,
    path: PathBuf,
    selected: usize,
    engine: Engine,
    device: Sender<Command>,
    events: Receiver<Event>,
    gate: InputGate,
    long_press: LongPress,
    long_editor: bool,
    wanted: bool,
    status: String,
    logs: VecDeque<String>,
    started: Instant,
    last_edit: Instant,
    base_profile: usize,
    foreground: Option<String>,
    auto_active: bool,
    sleep: SleepState,
    show_apps: bool,
    show_logs: bool,
    settings: bool,
    show_about: bool,
    update_result: Option<String>,
    update_rx: Option<Receiver<String>>,
    rename: bool,
    notice: String,
    error: bool,
    smoke: bool,
    frames: usize,
    capturing: bool,
    last_key: Option<(usize, Instant)>,
    key_previews: key_preview::Cache,
    plugins: Option<crate::plugin_runtime::Manager>,
    plugin_faces: HashMap<String, PluginFace>,
    plugin_errors: HashMap<String,String>,
    assistant: Option<crate::mcp_server::Server>,
    proposals: crate::assistant::Proposals,
    assistant_cache: HashMap<String, Result<serde_json::Value, String>>,
    assistant_settings: bool,
    connect_window: bool,
    http_child: Option<std::process::Child>,
    preferences: Preferences,
    preferences_valid: bool,
    catalog_tab: usize,
    catalog_search: String,
    catalog_category: String,
    catalog_selection: Option<usize>,
    save_bind: bool,
    bind_name: String,
    clipboard: Option<Key>,
    history: Vec<Config>,
    future: Vec<Config>,
    last_field: Option<egui::Id>,
    history_at: Instant,
    replaying: bool,
    drag: crate::gesture::Drag<DragItem>,
    drag_sources: Vec<(egui::Rect, DragItem)>,
    drop_targets: Vec<(egui::Rect, usize)>,
    recording: Option<(usize, String, usize, Vec<usize>, bool)>,
    recording_error: String,
    import_report: Option<crate::elgato::Report>,
    dialog_tx: Sender<Option<DialogEffect>>,
    dialog_rx: Receiver<Option<DialogEffect>>,
    dialog_pending: bool,
    page_manager: bool,
    delete_page: Option<String>,
    desktop: Option<crate::desktop::Desktop>,
    audio: crate::audio::Snapshot,
    obs_state: crate::obs_monitor::Snapshot,
    running_block: Vec<usize>,
    open_requested: bool,
    tray_ready: bool,
    quitting: bool,
    show_workflow: bool,
    show_assets: bool,
    show_desktop_settings: bool,
    available_icons: Vec<crate::assets::Icon>,
    available_plugins: Vec<crate::assets::Plugin>,
    icon_search: String,
    icon_pack: String,
    asset_tab: usize,
    trigger_recording: bool,
    icon_target: Option<icon_ui::Target>,
    icon_selected: Option<PathBuf>,
    icon_notice: String,
    assistant_request: String,
    flicker: Option<crate::flicker_diagnostic::Session>,
    flicker_path: Option<PathBuf>,
    diagnostic_deck: Option<egui::Rect>,
    diagnostic_inspector: Option<egui::Rect>,
    diagnostic_key_width: f32,
}
#[derive(Clone)]
enum DragItem {
    Bind(Key),
    Action(Key),
    Position(usize, String, usize),
    NewFolder,
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Runner {
        use raw_window_handle::HasWindowHandle;
        let app = Self::init(&cc.egui_ctx, None);
        if let Some(desktop) = &app.desktop {
            if let Ok(handle) = cc.window_handle() {
                if let raw_window_handle::RawWindowHandle::Win32(handle) = handle.as_raw() {
                    let _ = desktop
                        .tx
                        .send(crate::desktop::Command::Window(handle.hwnd.get()));
                }
            }
        }
        Runner::new(app, cc.egui_ctx.clone())
    }
    fn init(ctx: &egui::Context, override_path: Option<PathBuf>) -> Self {
        let isolated = override_path.is_some();
        theme::install(ctx);
        let path = override_path.unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap_or_else(|_| PathBuf::from("DeckForge.exe"))
                .with_file_name("config.json")
        });
        let mut load_error = None;
        let config = if path.exists() {
            store::load(&path).unwrap_or_else(|e| {
                load_error = Some(format!(
                    "Configuration illisible : {e}. Importe une sauvegarde pour la récupérer."
                ));
                Config::default()
            })
        } else {
            Config::default()
        };
        let smoke = !isolated && std::env::args().any(|a| a == "--ui-smoke");
        let preferences_result = Preferences::load(&path.with_file_name("preferences.json"));
        let preferences_valid = preferences_result.is_ok();
        let mut preferences = preferences_result.unwrap_or_default();
        if std::env::args().any(|a| a == "--light") {
            preferences.dark = false;
        }
        theme::apply(ctx, preferences.dark);
        let (tx, events) = mpsc::channel();
        let device = if isolated {
            mpsc::channel().0
        } else {
            device::spawn(tx.clone())
        };
        let desktop = if isolated || smoke {
            None
        } else {
            crate::audio::spawn(tx.clone());
            Some(crate::desktop::spawn(tx.clone(), ctx.clone()))
        };
        let engine = Engine::new(tx);
        let mut gate = InputGate::default();
        let ui_test = isolated || std::env::args().any(|a| a == "--ui-test");
        if ui_test {
            gate.enabled = false;
        }
        gate.invalid = load_error.is_some();
        let assistant = if isolated {
            None
        } else {
            crate::mcp_server::Server::new(path.with_file_name("mcp-connection.json")).ok()
        };
        if preferences_valid && !isolated && preferences.allow_connected_ai {
            if let Some(server) = &assistant { server.set_enabled(true); }
        }
        let (dialog_tx, dialog_rx) = mpsc::channel();
        let mut app = Self {
            applied: config.clone(),
            base_profile: config.active,
            config,
            foreground: None,
            auto_active: false,
            sleep: SleepState::new(Instant::now()),
            show_apps: false,
            pending: None,
            path,
            selected: 0,
            engine,
            device,
            events,
            gate,
            long_press: LongPress::default(),
            long_editor: false,
            wanted: false,
            status: "D6 non connecté".into(),
            logs: VecDeque::new(),
            started: Instant::now(),
            last_edit: Instant::now(),
            show_logs: false,
            settings: false,
            show_about: false,
            update_result: None,
            update_rx: None,
            rename: false,
            notice: "Tout est prêt.".into(),
            error: false,
            smoke,
            frames: 0,
            capturing: false,
            last_key: None,
            key_previews: key_preview::Cache::default(),
            plugin_faces: HashMap::new(),
            plugin_errors: HashMap::new(),
            plugins: if isolated || smoke { None } else { Some(crate::plugin_runtime::Manager::new(crate::plugin_import::installed_root().map(|root|crate::plugin_import::inventory(&root)).unwrap_or_default())) },
            assistant,
            proposals: Default::default(),
            assistant_cache: HashMap::new(),
            assistant_settings: false,
            connect_window: false,
            http_child: None,
            preferences,
            preferences_valid,
            catalog_tab: 1,
            catalog_search: String::new(),
            catalog_category: "Tout".into(),
            catalog_selection: Some(0),
            save_bind: false,
            bind_name: String::new(),
            clipboard: None,
            history: Vec::new(),
            future: Vec::new(),
            last_field: None,
            history_at: Instant::now(),
            replaying: false,
            drag: Default::default(),
            drag_sources: vec![],
            drop_targets: vec![],
            recording: None,
            recording_error: String::new(),
            import_report: None,
            dialog_tx,
            dialog_rx,
            dialog_pending: false,
            page_manager: false,
            delete_page: None,
            desktop,
            audio: Default::default(),
            obs_state: Default::default(),
            running_block: vec![],
            open_requested: false,
            tray_ready: false,
            quitting: false,
            show_workflow: false,
            show_assets: false,
            show_desktop_settings: false,
            available_icons: vec![],
            available_plugins: vec![],
            icon_search: String::new(),
            icon_pack: String::new(),
            asset_tab: 0,
            trigger_recording: false,
            icon_target: None,
            icon_selected: None,
            icon_notice: String::new(),
            assistant_request: String::new(),
            flicker: None, flicker_path: None, diagnostic_deck: None, diagnostic_inspector: None, diagnostic_key_width: 0.0,
        };
        if let Some(e) = load_error {
            app.fail(e);
        } else if (!ui_test && !smoke) || (!isolated && std::env::args().any(|a| a == "--connect"))
        {
            app.connect();
        }
        if !preferences_valid {
            app.fail("Préférences illisibles : le fichier est conservé. Restaure preferences.json.bak avant d’enregistrer Mes binds.".into());
        }
        app.refresh_hotkeys();
        app
    }
    fn log(&mut self, s: String) {
        while self.logs.len() >= 300 {
            self.logs.pop_front();
        }
        self.notice = s.clone();
        self.error = false;
        self.logs.push_back(format!(
            "{:>6.1}s  {s}",
            self.started.elapsed().as_secs_f32()
        ));
    }
    fn fail(&mut self, s: String) {
        self.log(s);
        self.error = true;
    }
    fn edited(&mut self) {
        self.long_press.clear();
        self.gate.dirty = true;
        self.last_edit = Instant::now();
        let was_sleeping = self.sleep.sleeping;
        self.sleep.activity(self.last_edit);
        if was_sleeping { self.wake_display(); }
    }
    fn upload(&mut self, connect: bool) {
        self.long_press.clear();
        let revision = self.gate.request_apply();
        let mut deployed = match self.config.deployment() {
            Ok(c) => c,
            Err(e) => {
                self.fail(e.to_string());
                return;
            }
        };
        for p in &mut deployed.profiles {
            for (page, keys) in std::iter::once(("", &mut p.keys)).chain(p.pages.iter_mut().map(|page| (page.id.as_str(), &mut page.keys)))
            {
                for (index, key) in keys.iter_mut().enumerate() {
                    *key = crate::obs_monitor::present_key(&crate::audio::present_key(key, &self.audio), &self.obs_state);
                    if let Some(badge) = crate::engine::toggle_badge(&crate::engine::identity(&p.name, page, index), key) { key.runtime_badge = Some(badge); }
                }
            }
        }
        self.pending = Some((revision, deployed.clone()));
        let command = if connect {
            Command::Connect(deployed.clone(), revision)
        } else {
            Command::Update(deployed, revision)
        };
        if self.device.send(command).is_err() {
            self.fail("Le service USB s’est arrêté. Relance DeckForge.".into());
            self.gate.disconnect();
        }
    }
    fn connect(&mut self) {
        if self.gate.invalid {
            return;
        }
        if let Err(e) = self.config.validate_structure() {
            self.fail(format!("Configuration à corriger : {e}"));
            return;
        }
        if self.gate.dirty {
            if let Err(e) = store::save(&self.path, &self.config) {
                self.fail(format!("Sauvegarde impossible : {e}"));
                return;
            }
        }
        self.wanted = true;
        self.upload(true);
        self.log("Connexion au D6…".into());
    }
    fn save(&mut self) {
        if self.gate.invalid {
            self.fail("Importe une configuration valide avant d’enregistrer.".into());
            return;
        }
        match store::save(&self.path, &self.config) {
            Ok(()) => {
                self.gate.dirty = false;
                self.refresh_hotkeys();
                if self.wanted {
                    self.upload(false);
                }
                self.log(if self.config.binding_errors().is_empty() {
                    "Modifications enregistrées, envoi au D6…".into()
                } else {
                    "Brouillons enregistrés. Synchronisation des touches valides…".into()
                });
            }
            Err(e) => self.fail(format!("Sauvegarde impossible : {e}")),
        }
    }
    fn execute_key(&mut self, key: Key, dry: bool, delayed: bool, config: &Config) {
        if let Err(e) = validate_key(&key, config) {
            self.fail(format!("Action à corriger : {e}"));
            return;
        }
        if key.steps.is_empty() {
            self.log(
                "Cette touche n’a pas encore d’action. Ajoute une action depuis la bibliothèque."
                    .into(),
            );
            return;
        }
        self.engine.start(key, dry, if delayed { 3000 } else { 0 }, config.obs.clone());
    }
    fn preview(&mut self, dry: bool) {
        let mut key = self.config.keys()[self.selected].clone();
        key.runtime_id = Some(crate::engine::identity(&self.config.profiles[self.config.active].name, &self.config.page, self.selected));
        let c = self.config.clone();
        self.execute_key(key, dry, !dry, &c);
    }
    fn execute_physical(&mut self, index: usize, long: bool) {
        if self.gate.decision() != Decision::Execute { return; }
        let c = self.applied.clone();
        let mut key = c.keys()[index].clone();
        key.runtime_id = Some(crate::engine::identity(&c.profiles[c.active].name, &c.page, index));
        if long { key.steps = std::mem::take(&mut key.long_steps); }
        self.execute_key(key, false, false, &c);
    }
    fn events(&mut self) {
        for _ in 0..1000 {
            let Ok(event) = self.events.try_recv() else {
                break;
            };
            match event {
                Event::Log(s) => self.log(s),
                Event::Notification(title, message) => {
                    self.log(format!("Notification — {title} : {message}"));
                    if self.tray_ready {
                        if let Some(desktop) = &self.desktop { let _ = desktop.tx.send(crate::desktop::Command::Notify(title, message)); }
                    }
                }
                Event::Block(path) => self.running_block = path,
                Event::D6Brightness(mode, value) => {
                    self.config.brightness = crate::model::d6_brightness(self.config.brightness, &mode, value);
                    if self.wanted && self.gate.connected && !self.sleep.sleeping {
                        let _ = self.device.send(Command::Brightness(self.config.brightness));
                    }
                    self.edited();
                }
                Event::ToggleVisual => { if self.wanted && self.gate.connected { self.upload(false); } },
                Event::D6Sleep => {
                    self.sleep.sleeping = true;
                    if self.wanted && self.gate.connected { let _ = self.device.send(Command::Brightness(self.config.sleep_brightness)); }
                }
                Event::Foreground(exe) => { self.foreground = exe; self.update_auto_profile(); }
                Event::Open => self.open_requested = true,
                Event::TrayReady(ready) => {
                    self.tray_ready = ready;
                    if !ready {
                        self.open_requested = true;
                    }
                }
                Event::Quit => {
                    self.quitting = true;
                    self.engine.stop();
                }
                Event::Toggle => {
                    self.gate.enabled = !self.gate.enabled;
                    self.engine.stop();
                }
                Event::Audio(snapshot) => {
                    let changed =
                        self.audio.output != snapshot.output || self.audio.input != snapshot.input;
                    self.audio = snapshot;
                    if changed && self.wanted && !self.gate.dirty {
                        self.upload(false);
                    }
                }
                Event::Hotkey(index, at) => {
                    if self.engine.allows_global_press(at)
                        && index < 15
                        && at.elapsed() < Duration::from_millis(500)
                        && self.gate.enabled
                        && !self.trigger_recording
                        && self.recording.is_none()
                        && !self.gate.invalid
                        && !self.gate.dirty
                    {
                        let c = self.config.clone();
                        let mut key = c.keys()[index].clone();
                        key.runtime_id = Some(crate::engine::identity(&c.profiles[c.active].name, &c.page, index));
                        self.execute_key(key, false, false, &c);
                    }
                }
                Event::Device(connected, message) => {
                    if self.status != message {
                        self.log(message.clone());
                    }
                    self.status = message;
                    if !connected {
                        self.long_press.clear();
                        self.gate.disconnect();
                        self.engine.stop();
                    } else {
                        self.gate.connected = true;
                    }
                }
                Event::Applied(revision) => {
                    if self.gate.applied(revision) {
                        self.long_press.clear();
                        if let Some((r, c)) = self.pending.take() {
                            if r == revision {
                                self.applied = c;
                                if let Some(plugins)=&mut self.plugins { plugins.sync(&self.applied); }
                                if self.sleep.sleeping {
                                    let _ = self.device.send(Command::Brightness(self.config.sleep_brightness));
                                }
                            }
                        }
                        self.log(
                            "D6 synchronisé. Les touches suivent le mode affiché en haut.".into(),
                        );
                    }
                }
                Event::Key(index, at) => {
                    let wake = self.sleep.press(Instant::now());
                    if wake { self.wake_display(); self.long_press.clear(); continue; }
                    self.last_key = Some((index, Instant::now()));
                    match if self.trigger_recording {
                        Decision::Blocked("Enregistrement du raccourci en cours")
                    } else {
                        self.gate.decision_for_age(at.elapsed())
                    } {
                        Decision::Inspect => {
                            self.selected = index;
                            self.log(format!(
                                "Diagnostic : touche {:02} reçue, aucune action exécutée.",
                                index + 1
                            ));
                        }
                        Decision::Blocked(reason) => {
                            self.log(format!("Touche {:02} : {reason}", index + 1))
                        }
                        Decision::Execute => {
                            if index < 15 && self.applied.keys()[index].plugin.is_some() { if let Some(plugins)=&self.plugins { plugins.key(index,true); } }
                            let has_long = !self.applied.keys()[index].long_steps.is_empty();
                            if self.long_press.press(index, at, has_long) {
                                self.execute_physical(index, false);
                            }
                        }
                    }
                }
                Event::KeyRelease(index, at) => {
                    if index < 15 {
                        if self.gate.decision()==Decision::Execute && self.applied.keys()[index].plugin.is_some() { if let Some(plugins)=&self.plugins { plugins.key(index,false); } }
                        if let Some(long) = self.long_press.release(index, at, self.applied.long_press_ms) {
                            self.execute_physical(index, long);
                        }
                    }
                }
                Event::Navigate(step) => {
                    self.navigate(step);
                }
                Event::Profile(name) => {
                    if let Some(index) = self.config.profiles.iter().position(|p| p.name == name) { self.select_profile(index); }
                }
            }
        }
    }
    fn navigate(&mut self, step: Step) {
        match self.config.navigate(&step) {
            Ok(()) => {
                self.last_field = None;
                self.selected = 0;
                self.save();
            }
            Err(e) => self.fail(format!("Navigation impossible : {e}")),
        }
    }
    fn create_page(&mut self, folder: bool) {
        if self.config.profiles[self.config.active].pages.len() >= 128 {
            self.fail("128 pages et dossiers maximum par profil.".into());
            return;
        }
        let id = format!(
            "page-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let prefix = if folder { "Dossier" } else { "Page" };
        let mut number = if folder { 1 } else { 2 };
        while self.config.profiles[self.config.active]
            .pages
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(&format!("{prefix} {number}")))
        {
            number += 1;
        }
        let name = format!("{prefix} {number}");
        let mut page = crate::model::Page::new(id.clone(), name.clone(), folder);
        if !folder {
            for (index, step, label) in [
                (13, Step::PagePrevious, "Page précédente"),
                (14, Step::PageNext, "Page suivante"),
            ] {
                page.keys[index] = Key {
                    label: label.into(),
                    color: [137, 165, 218],
                    steps: vec![step],
                    ..Key::default()
                };
                if self.config.keys()[index].steps.is_empty() {
                    self.config.keys_mut()[index] = page.keys[index].clone();
                }
            }
        }
        self.config.profiles[self.config.active].pages.push(page);
        if folder {
            self.assign(
                DragItem::Bind(Key {
                    label: name.into(),
                    color: [230, 175, 74],
                    steps: vec![Step::Folder { id: id.clone() }],
                    ..Key::default()
                }),
                self.selected,
            );
        } else {
            self.config.page = id;
            self.config.trail.clear();
            self.selected = 0;
            self.edited();
        }
    }
    fn remove_page(&mut self, id: &str) {
        let profile = &mut self.config.profiles[self.config.active];
        profile.pages.retain(|p| p.id != id);
        for keys in
            std::iter::once(&mut profile.keys).chain(profile.pages.iter_mut().map(|p| &mut p.keys))
        {
            for key in keys {
                if key
                    .steps
                    .iter()
                    .any(|s| matches!(s,Step::Page{id:target}|Step::Folder{id:target}if target==id))
                {
                    *key = Key::default();
                }
            }
        }
        if self.config.page == id {
            self.config.reset_navigation();
        } else {
            self.config.trail.retain(|p| p != id);
        }
        self.edited();
        self.log("Page supprimée et touches de navigation correspondantes retirées.".into());
    }
    fn record_events(&mut self, ctx: &egui::Context) {
        let Some((profile, page, key, step, enabled)) = self.recording.clone() else {
            return;
        };
        if !ctx.input(|i| i.focused) {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        if let Some(result) = crate::recorder::combination(&events) {
            ctx.input_mut(|i| i.events.clear());
            match result {
                Ok(text) => {
                    if let Some(Step::Hotkey { keys }) = self
                        .config
                        .profiles
                        .get_mut(profile)
                        .and_then(|p| p.keys_at_mut(&page))
                        .and_then(|keys| keys.get_mut(key))
                        .and_then(|key| extras::step_mut(&mut key.steps, &step))
                    {
                        *keys = text.clone();
                        self.edited();
                        self.log(format!("Raccourci enregistré : {text}"));
                    }
                    self.recording = None;
                    self.gate.enabled = enabled;
                    self.refresh_hotkeys();
                }
                Err(e) => self.recording_error = e,
            }
        }
    }
    fn select_profile(&mut self, index: usize) {
        self.base_profile = index;
        self.auto_active = false;
        if index != self.config.active {
            self.last_field = None;
            self.engine.stop();
            self.config.active = index;
            self.config.reset_navigation();
            self.selected = 0;
            self.save();
        }
    }
    fn update_auto_profile(&mut self) {
        if self.engine.running.load(Ordering::SeqCst) { return; }
        let target = choose_profile(&self.config, self.foreground.as_deref(), self.base_profile);
        self.auto_active = self.config.auto_profile && target != self.base_profile;
        if target != self.config.active {
            self.last_field = None;
            self.config.active = target;
            self.config.reset_navigation();
            self.selected = 0;
            self.save();
        }
    }
    fn wake_display(&mut self) {
        if self.wanted && self.gate.connected {
            let _ = self.device.send(Command::Brightness(self.config.brightness));
        }
    }
    fn housekeeping(&mut self) {
        self.update_auto_profile();
        if self.wanted && self.gate.connected && self.sleep.due(Instant::now(), self.config.sleep_after_min) {
            let _ = self.device.send(Command::Brightness(self.config.sleep_brightness));
        }
    }
    fn add_profile(&mut self, duplicate: bool) {
        let mut n = self.config.profiles.len() + 1;
        while self
            .config
            .profiles
            .iter()
            .any(|p| p.name == format!("Profil {n}"))
        {
            n += 1;
        }
        let mut p = if duplicate {
            self.config.profiles[self.config.active].clone()
        } else {
            Profile::new(String::new())
        };
        p.name = format!("Profil {n}");
        self.config.profiles.push(p);
        self.config.active = self.config.profiles.len() - 1;
        self.base_profile = self.config.active;
        self.auto_active = false;
        self.config.reset_navigation();
        self.selected = 0;
        self.edited();
    }
    fn start_dialog(&mut self, task: impl FnOnce() -> Option<DialogEffect> + Send + 'static) {
        if self.dialog_pending { return; }
        self.dialog_pending = true;
        let tx = self.dialog_tx.clone();
        std::thread::spawn(move || { let _ = tx.send(task()); });
    }
    fn dialog_results(&mut self) {
        while let Ok(result) = self.dialog_rx.try_recv() {
            self.dialog_pending = false;
            if let Some(effect) = result { effect(self); }
        }
    }
    fn import(&mut self) {
        self.gate.enabled = false;
        self.engine.stop();
        self.start_dialog(|| {
            let path = rfd::FileDialog::new()
                .add_filter("Configuration DeckForge", &["json", "bak"]).pick_file()?;
            let loaded = store::load(&path);
            Some(Box::new(move |app| match loaded {
                Ok(c) => {
                    app.config = c;
                    app.base_profile = app.config.active;
                    app.auto_active = false;
                    app.selected = 0;
                    app.gate.invalid = false;
                    app.edited();
                    app.log("Profils importés. Les touches restent en pause jusqu'à leur réactivation.".into());
                }
                Err(e) => app.fail(format!("Import impossible : {e}")),
            }))
        });
    }
    fn export(&mut self) {
        self.gate.enabled = false;
        let config = self.config.clone();
        self.start_dialog(move || {
            let path = rfd::FileDialog::new().set_file_name("deckforge-profils.json")
                .add_filter("JSON", &["json"]).save_file()?;
            let result = store::save(&path, &config);
            Some(Box::new(move |app| match result {
                Ok(()) => app.log("Profils exportés. Réactive les touches quand tu es prêt.".into()),
                Err(e) => app.fail(format!("Export impossible : {e}")),
            }))
        });
    }
    fn assistant_events(&mut self) {
        if self
            .http_child
            .as_mut()
            .is_some_and(|child| child.try_wait().ok().flatten().is_some())
        {
            self.http_child = None;
            self.fail("Mode HTTP arrêté. Vérifie que le port local 9846 est disponible.".into());
        }
        loop {
            let call = match self
                .assistant
                .as_ref()
                .and_then(|s| s.requests.try_recv().ok())
            {
                Some(c) => c,
                None => break,
            };
            let result = if !self.assistant.as_ref().is_some_and(|s| s.enabled()) {
                Err("Assistant IA est désactivé".into())
            } else if let Some(cached) = self.assistant_cache.get(&call.id) {
                cached.clone()
            } else {
                let result = self.assistant_call(&call.name, &call.arguments);
                if self.assistant_cache.len() >= 128 {
                    self.assistant_cache.clear();
                }
                self.assistant_cache.insert(call.id.clone(), result.clone());
                result
            };
            let _ = call.reply.send(result);
        }
    }
    fn assistant_call(
        &mut self,
        name: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        use serde_json::json;
        let tools = crate::mcp_protocol::tools();
        let schema = tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .ok_or("Outil inconnu")?;
        let map = args
            .as_object()
            .ok_or("Les arguments doivent être un objet JSON")?;
        let allowed = schema["inputSchema"]["properties"].as_object().unwrap();
        if map.keys().any(|k| !allowed.contains_key(k)) {
            return Err("Argument non pris en charge".into());
        }
        if let Some(required) = schema["inputSchema"]["required"].as_array() {
            if required
                .iter()
                .any(|r| !map.contains_key(r.as_str().unwrap()))
            {
                return Err("Argument requis manquant".into());
            }
        }
        self.log(format!("Assistant IA : {name}"));
        match name {
            "deckforge_get_icons" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(30);
                if !(1..=50).contains(&limit) || query.len() > 128 {
                    return Err("Recherche invalide : 1 à 50 icônes, 128 caractères maximum".into());
                }
                if self.available_icons.is_empty() {
                    self.available_icons =
                        crate::assets::scan(&[self.path.parent().unwrap().join("assets")]);
                }
                let icons = self
                    .available_icons
                    .iter()
                    .filter(|i| {
                        format!("{} {}", i.name, i.pack)
                            .to_lowercase()
                            .contains(&query)
                    })
                    .take(limit as usize)
                    .map(|i| json!({"name":i.name,"pack":i.pack,"path":i.path}))
                    .collect::<Vec<_>>();
                Ok(
                    json!({"icons":icons,"audio_defaults":crate::audio::BUILTIN_ICONS.iter().map(|(id,name,_)|json!({"id":id,"name":name})).collect::<Vec<_>>(),"usage":"For audio_switch set first_icon and second_icon to a returned path or builtin id; an empty icon selects the default headphones/speakers. Propose the binding with deckforge_configure_key."}),
                )
            }
            "deckforge_get_state" => Ok(
                json!({"connected":self.gate.connected,"physical_keys":format!("{:?}",self.gate.decision()),"current_profile":self.config.profiles[self.config.active].name,"current_page":self.config.page,"trail":self.config.trail,"profiles":self.config.profiles.iter().map(|p|json!({"name":p.name,"pages":p.pages.iter().map(|page|json!({"id":page.id,"name":page.name,"folder":page.folder})).collect::<Vec<_>>(),"keys":p.keys.iter().enumerate().map(|(i,k)|json!({"key":i+1,"label":k.label,"steps":k.steps.len()})).collect::<Vec<_>>()})).collect::<Vec<_>>(),"pending_proposals":self.proposals.pending.len(),"configuration_invalid":self.gate.invalid}),
            ),
            "deckforge_get_audio_devices" => Ok(serde_json::to_value(&self.audio).unwrap()),
            "deckforge_get_profile" => {
                let name = args["profile"].as_str().ok_or("Nom de profil requis")?;
                let profile = self
                    .config
                    .profiles
                    .iter()
                    .find(|p| p.name == name)
                    .ok_or("Profil introuvable")?;
                let value = serde_json::to_value(profile).unwrap();
                if value.to_string().len() > crate::mcp_protocol::MAX_FRAME - 1024 {
                    return Err("Ce profil est trop volumineux pour le connecteur".into());
                }
                Ok(json!({"profile":value}))
            }
            "deckforge_create_profile"
            | "deckforge_create_page"
            | "deckforge_configure_key"
            | "deckforge_request_run" => {
                if self.gate.invalid {
                    return Err("Récupère d’abord la configuration dans DeckForge".into());
                }
                self.proposals.propose(name, args, &self.config)
            }
            "deckforge_simulate_macro" => {
                let (p, page, k) = crate::assistant::position(&self.config, args)?;
                let c = self.config.clone();
                let key = c.profiles[p].keys_at(&page).unwrap()[k].clone();
                self.execute_key(key, true, false, &c);
                Ok(json!({"status":"simulation_requested","effects_on_pc":false}))
            }
            "deckforge_stop_macro" => {
                self.engine.stop();
                self.gate.enabled = false;
                Ok(json!({"status":"stop_requested","physical_keys_paused":true}))
            }
            "deckforge_proposal_status" => self.proposals.status(args),
            _ => Err("Outil inconnu".into()),
        }
    }
    fn start_http(&mut self) {
        use std::os::windows::process::CommandExt;
        if !self.assistant.as_ref().is_some_and(|s| s.enabled()) {
            self.fail("Active d’abord Assistant IA.".into());
            return;
        }
        if self.http_child.is_some() {
            return;
        }
        match std::process::Command::new(self.path.with_file_name("deckforge-mcp.exe"))
            .args(["--http", "127.0.0.1:9846"])
            .creation_flags(0x08000000)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => {
                self.http_child = Some(child);
                self.log("Mode MCP HTTP local activé sur le port 9846.".into());
            }
            Err(e) => self.fail(format!("Mode HTTP indisponible : {e}")),
        }
    }
    fn stop_http(&mut self) {
        if let Some(mut child) = self.http_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    fn set_ai_enabled(&mut self, enabled: bool) {
        if let Some(server)=&self.assistant { server.set_enabled(enabled); }
        if !enabled { self.stop_http(); self.proposals.reject_all(); self.assistant_cache.clear(); }
        self.preferences.allow_connected_ai=enabled;
        self.save_preferences();
    }
    fn connect_ai_window(&mut self, ctx: &egui::Context) {
        if !self.connect_window { return; }
        let mut open=true;
        egui::Window::new("Connecter une IA").open(&mut open).default_width(530.0).resizable(true).vscroll(true).show(ctx,|ui|{
            ui.label("Choisis ton application IA. DeckForge n'écrit jamais de clé dans ses réglages.");
            let locations=crate::mcp_connect::Locations::system();
            let exe=crate::mcp_connect::connector(&self.path);
            for client in [crate::mcp_connect::Client::Desktop,crate::mcp_connect::Client::Code,crate::mcp_connect::Client::Codex,crate::mcp_connect::Client::Cursor] {
                egui::Frame::group(ui.style()).show(ui,|ui|{
                    ui.label(RichText::new(client.name()).strong().size(17.0));
                    let configured=locations.as_ref().is_ok_and(|loc|crate::mcp_connect::configured(client,loc));
                    ui.label(if configured {"Configuré sur ce PC"} else {"Pas encore configuré"});
                    ui.horizontal(|ui|{
                        if ui.add_enabled(exe.is_ok() && locations.is_ok(),egui::Button::new("Configurer automatiquement")).clicked() {
                            match if client==crate::mcp_connect::Client::Code {crate::mcp_connect::configure_code(locations.as_ref().unwrap(),exe.as_ref().unwrap(),false)} else {crate::mcp_connect::configure(client,locations.as_ref().unwrap(),exe.as_ref().unwrap(),false)}{Ok(msg)=>self.log(msg),Err(err)=>self.fail(err)}
                        }
                        if ui.add_enabled(configured,egui::Button::new("Retirer")).clicked() {
                            match if client==crate::mcp_connect::Client::Code {crate::mcp_connect::configure_code(locations.as_ref().unwrap(),&self.path,true)} else {crate::mcp_connect::configure(client,locations.as_ref().unwrap(),&self.path,true)}{Ok(msg)=>self.log(msg),Err(err)=>self.fail(err)}
                        }
                    });
                });ui.add_space(8.0);
            }
            if let Err(e)=&exe { ui.colored_label(Color32::DARK_RED,e); }
            egui::Frame::group(ui.style()).show(ui,|ui|{
                ui.label(RichText::new("ChatGPT : Secure MCP Tunnel").strong());
                ui.label("1. Ouvre la documentation du tunnel. 2. Copie le chemin du connecteur stdio à fournir au tunnel. 3. Dans ChatGPT : Plugins, Ajouter un serveur MCP, puis Tunnel. Disponibilité à vérifier selon ton compte. Le port HTTP reste local.");
                if ui.button("Ouvrir la page du tunnel").clicked(){let _={ui.ctx().open_url(egui::OpenUrl::new_tab("https://developers.openai.com/api/docs/guides/secure-mcp-tunnels"));Ok::<(),String>(())};}
                if ui.button("Copier la commande stdio").clicked(){ui.ctx().copy_text(self.path.with_file_name("deckforge-mcp.exe").to_string_lossy().into_owned());}
            });
            if ui.button("Autre client : copier le JSON standard").clicked(){
                let config=serde_json::json!({"mcpServers":{"deckforge":{"command":self.path.with_file_name("deckforge-mcp.exe").to_string_lossy(),"args":[]}}});
                ui.ctx().copy_text(serde_json::to_string_pretty(&config).unwrap());
            }
            if ui.button("Tester la connexion").clicked(){
                let result=if !self.assistant.as_ref().is_some_and(|s|s.enabled()) {Err("Assistant désactivé : active d'abord Autoriser les IA connectées".into())} else {crate::mcp_connect::test_connection(&self.path)};
                match result {Ok(msg)=>self.log(msg),Err(e)=>self.fail(e)}
            }
            ui.label(&self.notice);
        });
        self.connect_window=open;
    }
    fn assistant_window(&mut self, ctx: &egui::Context) {
        let p = theme::palette(ctx);
        if self.assistant_settings {
            let mut open = true;
            egui::Window::new("Assistant IA").open(&mut open).collapsible(false).resizable(true).default_width(510.0).vscroll(true).max_height((ctx.screen_rect().height()-100.0).max(400.0)).show(ctx,|ui|{
            ui.label(RichText::new("Prépare tes macros avec ton IA.").size(20.0).strong());ui.add_space(10.0);
            let mut enabled=self.assistant.as_ref().is_some_and(|s|s.enabled());
            ui.horizontal(|ui|{if theme::toggle(ui,&mut enabled).changed(){if let Some(server)=&self.assistant{server.set_enabled(enabled);}if !enabled{self.stop_http();self.proposals.reject_all();self.assistant_cache.clear();}self.preferences.allow_connected_ai=enabled;self.save_preferences();self.log(if enabled{"Connecteur MCP activé."}else{"Connecteur MCP désactivé."}.into());}ui.label(RichText::new(if enabled{"Assistant IA activé"}else{"Assistant IA désactivé"}).strong());});
            ui.add_space(10.0);ui.label("Ton assistant peut lire les profils et proposer des actions. Les changements et les exécutions réelles attendent toujours ta validation ici.");ui.add_space(8.0);
            ui.separator();ui.label(RichText::new("Décris l’automatisation souhaitée").strong());
            ui.add(egui::TextEdit::multiline(&mut self.assistant_request).desired_rows(3).desired_width(f32::INFINITY).hint_text("Ex. : basculer entre mon casque et mes enceintes, avec une icône pour chaque état."));
            ui.label(RichText::new(format!("Destination : {} · touche {:02}",self.config.profiles[self.config.active].name,self.selected+1)).small());
            if ui.add_enabled(!self.assistant_request.trim().is_empty(),egui::Button::new("Copier la demande pour mon IA")).clicked(){ui.ctx().copy_text(self.automation_prompt());self.log("Demande copiée. Colle-la dans ton IA connectée au MCP DeckForge.".into());}
            ui.label(RichText::new("Ton IA doit être connectée au MCP. Elle proposera les blocs ici ; tu pourras les valider puis les modifier dans l’Atelier. Aucun modèle local ni appel d’IA n’est lancé par ce champ.").small().color(p.muted));
            ui.separator();
            if self.assistant.is_none(){ui.colored_label(Color32::DARK_RED,"Le connecteur local n’a pas pu démarrer.");}
            ui.label(RichText::new("Connexion locale MCP · Claude, Codex et clients compatibles stdio. Aucune clé d’IA à saisir dans DeckForge.").small().color(p.muted));
            if ui.button("Connecter une IA").clicked(){self.connect_window=true;}
            if ui.button("Copier la configuration MCP").clicked(){let exe=self.path.with_file_name("deckforge-mcp.exe");let config=serde_json::json!({"mcpServers":{"deckforge":{"command":exe.to_string_lossy(),"args":[]}}});ui.ctx().copy_text(serde_json::to_string_pretty(&config).unwrap());self.log("Configuration MCP copiée.".into());}
            ui.label(RichText::new("Le guide MCP fourni avec l’application détaille la connexion à ChatGPT et aux autres clients.").small().color(p.muted));
            ui.add_space(14.0);ui.separator();ui.label(RichText::new("ChatGPT web / clients HTTP").strong());
            ui.label(RichText::new("Le mode HTTP reste local au PC. Un tunnel ou un proxy compatible est nécessaire pour un service IA distant.").small().color(p.muted));
            ui.horizontal(|ui|{if ui.add_enabled(enabled,egui::Button::new(if self.http_child.is_some(){"Arrêter HTTP"}else{"Démarrer HTTP"})).clicked(){if self.http_child.is_some(){self.stop_http();}else{self.start_http();}}if self.http_child.is_some()&&ui.button("Copier l’adresse").clicked(){ui.ctx().copy_text("http://127.0.0.1:9846/mcp".into());}});
            if self.http_child.is_some()&&ui.button("Copier la clé HTTP").clicked(){match std::fs::read_to_string(self.path.with_file_name("mcp-http-token.txt")){Ok(t)=>{ui.ctx().copy_text(t.trim().into());self.log("Clé HTTP copiée. Utilise-la uniquement dans ton client MCP.".into());},Err(_)=>self.fail("La clé n’est pas encore prête. Réessaie dans un instant.".into())}}
        });
            self.assistant_settings = open;
        }
        self.connect_ai_window(ctx);
        let mut decision = None;
        if let Some(p) = self.proposals.pending.front() {
            egui::Window::new("Proposition de ton assistant").id(egui::Id::new("ai_review")).collapsible(false).resizable(true).default_width(570.0).show(ctx,|ui|{
                theme::pill(ui,"VALIDATION NÉCESSAIRE",BLUE);ui.add_space(8.0);ui.label(RichText::new(&p.title).size(18.0).strong());
                ui.label(RichText::new("Aucun changement ni lancement n’a encore eu lieu. Vérifie les actions avant de continuer.").color(theme::palette(ctx).muted));
                egui::ScrollArea::vertical().max_height(360.0).show(ui,|ui|{ui.label(&p.details);});ui.add_space(12.0);
                ui.horizontal(|ui|{if ui.button("Refuser").clicked(){decision=Some(false);}if ui.add(theme::primary(RichText::new("Valider cette proposition").color(Color32::WHITE))).clicked(){decision=Some(true);}});
            });
        }
        if let Some(accept) = decision {
            match self.proposals.decide(accept, &self.config) {
                Ok(Some(crate::assistant::Change::Config(c))) => {
                    self.engine.stop();
                    self.config = c;
                    self.selected = 0;
                    self.edited();
                    self.log(
                        "Proposition appliquée. Sauvegarde et synchronisation en cours.".into(),
                    );
                }
                Ok(Some(crate::assistant::Change::Run(key))) => {
                    let c = self.config.clone();
                    self.execute_key(key, false, true, &c);
                }
                Ok(None) => self.log("Proposition refusée.".into()),
                Err(e) => self.fail(e),
            }
        }
    }
    fn screenshot(&mut self, ctx: &egui::Context) {
        if !self.smoke {
            return;
        }
        if !self.capturing
            && self.frames >= 20
            && (!self.wanted || self.gate.connected || self.frames >= 100)
        {
            self.capturing = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        if let Some(img) = ctx.input(|i| {
            i.events.iter().find_map(|e| {
                if let egui::Event::Screenshot { image, .. } = e {
                    Some(image.clone())
                } else {
                    None
                }
            })
        }) {
            let bytes = img
                .pixels
                .iter()
                .flat_map(|p| p.to_array())
                .collect::<Vec<_>>();
            let name = if std::env::args().any(|a| a == "--small") {
                "preview-small.png"
            } else {
                "preview.png"
            };
            let _ = image::save_buffer(
                self.path.with_file_name(name),
                &bytes,
                img.size[0] as u32,
                img.size[1] as u32,
                image::ExtendedColorType::Rgba8,
            );
            let report = serde_json::json!({"rendered":true,"connected":self.gate.connected,"physical_keys":format!("{:?}",self.gate.decision()),"version":env!("CARGO_PKG_VERSION"),"dark":self.preferences.dark,"catalog_binds":catalog::presets().len(),"image_size":img.size,"assistant_enabled":self.assistant.as_ref().is_some_and(|s|s.enabled())});
            let _ = std::fs::write(
                self.path.with_file_name("ui-smoke.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.frames > 140 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        let p = theme::palette(ctx);
        let compact = ctx.screen_rect().width() < 1150.0;
        let width = if compact { 100.0 } else { 200.0 };
        egui::SidePanel::left("navigation")
            .exact_width(width)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(p.sidebar)
                    .inner_margin(egui::Margin::symmetric(if compact { 6 } else { 12 }, 12)),
            )
            .show(ctx, |ui| {
                let footer_h = if compact { 54.0 } else { 64.0 };
                let scroll_max = (ui.available_height() - footer_h - 8.0).max(80.0);
                egui::ScrollArea::vertical()
                    .id_salt("sidebar_scroll")
                    .max_height(scroll_max)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if !compact {
                                theme::section_title(ui, "PROFILS");
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.menu_button("•••", |ui| {
                                        if ui
                                            .add_enabled(
                                                self.config.profiles.len() < 64,
                                                egui::Button::new("Nouveau profil"),
                                            )
                                            .clicked()
                                        {
                                            self.add_profile(false);
                                            ui.close_menu();
                                        }
                                        if ui.button("Renommer / dupliquer").clicked() {
                                            self.rename = true;
                                            ui.close_menu();
                                        }
                                        if ui.button("Applications associées").clicked() {
                                            self.show_apps = true;
                                            ui.close_menu();
                                        }
                                    });
                                    if ui
                                        .add_enabled(
                                            self.config.profiles.len() < 64,
                                            egui::Button::new("+").small(),
                                        )
                                        .on_hover_text("Nouveau profil")
                                        .clicked()
                                    {
                                        self.add_profile(false);
                                    }
                                },
                            );
                        });
                        ui.add_space(4.0);
                        let mut next_profile = None;
                        for (i, profile) in self.config.profiles.iter().enumerate() {
                            let selected = i == self.config.active;
                            let (r, response) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 34.0),
                                egui::Sense::click(),
                            );
                            if selected || response.hovered() {
                                ui.painter().rect_filled(
                                    r,
                                    10.0,
                                    if selected { p.wash } else { p.hover },
                                );
                            }
                            let icon_color = if selected { BLUE } else { p.muted };
                            let text_color = if selected { p.ink } else { p.muted };
                            let icon_pos = if compact {
                                r.center()
                            } else {
                                r.left_center() + Vec2::new(16.0, 0.0)
                            };
                            theme::symbol(ui.painter(), icon_pos, 15.0, 15, icon_color);
                            if !compact {
                                let mut job = egui::text::LayoutJob::simple_singleline(
                                    profile.name.clone(),
                                    egui::FontId::proportional(13.0),
                                    text_color,
                                );
                                job.wrap.max_width = (r.width() - 36.0).max(20.0);
                                job.wrap.max_rows = 1;
                                let galley = ui.painter().layout_job(job);
                                ui.painter().galley(
                                    egui::pos2(
                                        r.min.x + 32.0,
                                        r.center().y - galley.size().y * 0.5,
                                    ),
                                    galley,
                                    text_color,
                                );
                            }
                            let response = response.on_hover_text(&profile.name);
                            if response.clicked() && !selected {
                                next_profile = Some(i);
                            }
                        }
                        if let Some(i) = next_profile {
                            self.select_profile(i);
                        }

                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            if !compact {
                                theme::section_title(ui, "PAGES");
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.menu_button("+", |ui| {
                                        if ui.button("Nouvelle page").clicked() {
                                            self.create_page(false);
                                            ui.close_menu();
                                        }
                                        if ui.button("Gérer pages et dossiers…").clicked() {
                                            self.page_manager = true;
                                            ui.close_menu();
                                        }
                                        if ui
                                            .button("Dossier sur la touche sélectionnée")
                                            .clicked()
                                        {
                                            self.create_page(true);
                                            ui.close_menu();
                                        }
                                    });
                                },
                            );
                        });
                        ui.add_space(4.0);
                        let mut pages: Vec<(String, String)> =
                            vec![(String::new(), "Page 1".into())];
                        for page in self.config.profiles[self.config.active]
                            .pages
                            .iter()
                            .filter(|p| !p.folder)
                        {
                            pages.push((page.id.clone(), page.name.clone()));
                        }
                        let mut next_page = None;
                        for (idx, (id, name)) in pages.iter().enumerate() {
                            let selected = *id == self.config.page;
                            let (r, response) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 32.0),
                                egui::Sense::click(),
                            );
                            if selected || response.hovered() {
                                ui.painter().rect_filled(
                                    r,
                                    10.0,
                                    if selected { p.wash } else { p.hover },
                                );
                            }
                            let dot_color = if selected { BLUE } else { p.muted };
                            let text_color = if selected { p.ink } else { p.muted };
                            if compact {
                                ui.painter().text(
                                    r.center(),
                                    egui::Align2::CENTER_CENTER,
                                    format!("{}", idx + 1),
                                    egui::FontId::proportional(12.0),
                                    text_color,
                                );
                            } else {
                                ui.painter().circle_filled(
                                    r.left_center() + Vec2::new(16.0, 0.0),
                                    3.0,
                                    dot_color,
                                );
                                let mut job = egui::text::LayoutJob::simple_singleline(
                                    name.clone(),
                                    egui::FontId::proportional(12.5),
                                    text_color,
                                );
                                job.wrap.max_width = (r.width() - 34.0).max(20.0);
                                job.wrap.max_rows = 1;
                                let galley = ui.painter().layout_job(job);
                                ui.painter().galley(
                                    egui::pos2(
                                        r.min.x + 28.0,
                                        r.center().y - galley.size().y * 0.5,
                                    ),
                                    galley,
                                    text_color,
                                );
                            }
                            let response = response.on_hover_text(name);
                            if response.clicked() && !selected {
                                next_page = Some(id.clone());
                            }
                        }
                        if let Some(id) = next_page {
                            self.navigate(Step::Page { id });
                        }
                    });

                let spacer = (ui.available_height() - footer_h).max(0.0);
                ui.add_space(spacer);
                let (status_label, status_color) = if self.gate.connected {
                    ("Connecté", GREEN)
                } else if self.wanted {
                    ("En attente", theme::WARNING)
                } else {
                    ("Déconnecté", theme::DANGER)
                };
                let (card_r, card_resp) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), footer_h - 4.0),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(card_r, 10.0, p.card);
                if compact {
                    ui.painter().circle_filled(
                        card_r.center() - Vec2::new(0.0, 7.0),
                        4.0,
                        status_color,
                    );
                    ui.painter().text(
                        card_r.center() + Vec2::new(0.0, 8.0),
                        egui::Align2::CENTER_CENTER,
                        "D6",
                        egui::FontId::proportional(10.0),
                        p.ink,
                    );
                    card_resp.on_hover_text(format!("FIFINE D6 · 15 touches · {status_label}"));
                } else {
                    ui.painter().text(
                        card_r.min + Vec2::new(12.0, 10.0),
                        egui::Align2::LEFT_TOP,
                        "FIFINE D6 · 15 touches",
                        egui::FontId::proportional(11.0),
                        p.ink,
                    );
                    let dot_center = card_r.min + Vec2::new(16.0, 36.0);
                    ui.painter().circle_filled(dot_center, 4.0, status_color);
                    ui.painter().text(
                        egui::pos2(dot_center.x + 8.0, dot_center.y),
                        egui::Align2::LEFT_CENTER,
                        status_label,
                        egui::FontId::proportional(10.5),
                        p.muted,
                    );
                }
            });
    }
    fn header(&mut self, ctx: &egui::Context) {
        let p = theme::palette(ctx);
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .inner_margin(egui::Margin::symmetric(18, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 7.0, BLUE);
                    for y in 0..2 {
                        for x in 0..3 {
                            ui.painter().circle_filled(
                                r.min + Vec2::new(7.5 + x as f32 * 6.5, 9.5 + y as f32 * 8.0),
                                1.8,
                                Color32::WHITE,
                            );
                        }
                    }
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(RichText::new("DeckForge").size(18.0).strong());
                        ui.label(RichText::new("FIFINE D6").size(10.0).color(p.muted));
                    });
                    ui.add_space(8.0);
                    theme::status_badge(
                        ui,
                        if self.gate.connected {
                            "D6 connecté"
                        } else if self.wanted {
                            "D6 en attente"
                        } else {
                            "D6 déconnecté"
                        },
                        if self.gate.connected {
                            GREEN
                        } else if self.wanted {
                            theme::WARNING
                        } else {
                            theme::DANGER
                        },
                    );
                    ui.add_space(4.0);
                    if ui
                        .add_enabled(!self.history.is_empty(), theme::secondary_button(if ctx.screen_rect().width() < 1320.0 { "↺" } else { "Annuler" }))
                        .on_hover_text("Annuler — Ctrl+Z")
                        .clicked()
                    {
                        self.undo_edit();
                    }
                    if ui
                        .add_enabled(!self.future.is_empty(), theme::secondary_button(if ctx.screen_rect().width() < 1320.0 { "↻" } else { "Rétablir" }))
                        .on_hover_text("Rétablir — Ctrl+Y")
                        .clicked()
                    {
                        self.redo_edit();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button("Réglages", |ui| {
                            if ui.button("Windows et raccourcis…").clicked() {
                                self.show_desktop_settings = true;
                                ui.close_menu();
                            }
                            if ui.button("Icônes et extensions…").clicked() {
                                self.reload_icons();
                                self.show_assets = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Quitter DeckForge").clicked() {
                                self.quitting = true;
                                ui.close_menu();
                            }
                            if ui.button("Importer depuis Stream Deck…").clicked() {
                                self.import_elgato();
                                ui.close_menu();
                            }
                            if ui.button("Réglages du D6").clicked() {
                                self.settings = true;
                                ui.close_menu();
                            }
                            if ui.button("À propos de DeckForge").clicked() {
                                self.show_about = true;
                                ui.close_menu();
                            }
                            if ui.button("Assistant IA / MCP").clicked() {
                                self.assistant_settings = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Importer des profils…").clicked() {
                                self.import();
                                ui.close_menu();
                            }
                            if ui.button("Exporter les profils…").clicked() {
                                self.export();
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui
                                .button(if self.wanted {
                                    "Déconnecter le D6"
                                } else {
                                    "Connecter le D6"
                                })
                                .clicked()
                            {
                                if self.wanted {
                                    self.wanted = false;
                                    let _ = self.device.send(Command::Disconnect);
                                    self.gate.disconnect();
                                    self.engine.stop();
                                } else {
                                    self.connect();
                                }
                                ui.close_menu();
                            }
                        });
                        let ai_on = self.assistant.as_ref().is_some_and(|s| s.enabled());
                        let ai_label = if ai_on {
                            "IA : connectée"
                        } else if self.preferences.allow_connected_ai {
                            "IA : en attente"
                        } else {
                            "IA : désactivée"
                        };
                        if ui.add(theme::secondary_button("Connecter une IA")).clicked() {
                            self.connect_window = true;
                        }
                        if ui
                            .add(theme::secondary_button(ai_label))
                            .on_hover_text("Connecter une IA ou désactiver le pont local")
                            .clicked()
                        {
                            if ai_on {
                                self.set_ai_enabled(false);
                            } else {
                                self.assistant_settings = true;
                            }
                        }
                        if ui
                            .add(theme::secondary_button(if self.preferences.dark {
                                "Thème clair"
                            } else {
                                "Thème sombre"
                            }))
                            .on_hover_text("Changer le thème de toute l’interface")
                            .clicked()
                        {
                            self.preferences.dark = !self.preferences.dark;
                            theme::apply(ctx, self.preferences.dark);
                            self.save_preferences();
                        }
                        if ui
                            .add(
                                theme::danger_button(
                                    RichText::new("Arrêter les macros en cours")
                                        .color(theme::DANGER),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(egui::Stroke::new(1.0_f32, theme::DANGER)),
                            )
                            .on_hover_text("Arrête les macros en cours et met les touches en pause. Les applications déjà ouvertes restent ouvertes.")
                            .clicked()
                        {
                            self.engine.stop();
                            self.gate.enabled = false;
                            self.log("Macros arrêtées. Touches en pause.".into());
                        }
                        if theme::toggle(ui, &mut self.gate.enabled).changed() {
                            if !self.gate.enabled {
                                self.engine.stop();
                            } else {
                                self.gate.inspect = false;
                            }
                        }
                        let (label, color) = match self.gate.decision() {
                            Decision::Execute => ("Touches actives", GREEN),
                            Decision::Inspect => ("Diagnostic", BLUE),
                            Decision::Blocked(_) => (
                                if !self.gate.enabled {
                                    "En pause"
                                } else if !self.gate.connected {
                                    if self.wanted {
                                        "D6 en attente"
                                    } else {
                                        "Déconnecté"
                                    }
                                } else if self.gate.dirty {
                                    "Modifications…"
                                } else {
                                    "Synchronisation…"
                                },
                                p.muted,
                            ),
                        };
                        ui.label(RichText::new(label).size(12.0).strong().color(color));
                    });
                });
            });
    }
    fn save_preferences(&mut self) -> bool {
        if !self.preferences_valid {
            self.fail("Le fichier preferences.json est illisible et reste préservé. Restaure sa sauvegarde avant d’enregistrer.".into());
            return false;
        }
        match self
            .preferences
            .save(&self.path.with_file_name("preferences.json"))
        {
            Ok(()) => true,
            Err(e) => {
                self.fail(format!("Enregistrement de Mes binds impossible : {e}"));
                false
            }
        }
    }
    fn assign(&mut self, item: DragItem, target: usize) {
        if matches!(item, DragItem::NewFolder) {
            self.selected = target;
            self.create_page(true);
            return;
        }
        let active = self.config.active;
        let before = self.config.keys().clone();
        let page = self.config.page.clone();

        match item {
            DragItem::Position(profile, origin, from)
                if profile == active
                    && origin == page
                    && from < 15
                    && target < 15
                    && from != target =>
            {
                self.config.keys_mut().swap(from, target);
            }
            DragItem::Position(_, _, _) => return,
            DragItem::NewFolder => unreachable!(),
            DragItem::Bind(key) => {
                if let Err(e) = crate::model::validate_key_in_profile(
                    &key,
                    &self.config,
                    &self.config.profiles[active],
                ) {
                    self.fail(format!("Bind incompatible : {e}"));
                    return;
                }
                self.config.keys_mut()[target] = key;
            }
            DragItem::Action(key) => {
                let current = &mut self.config.keys_mut()[target];
                if current.steps.len() >= 256 {
                    self.fail("Une macro contient au maximum 256 étapes.".into());
                    return;
                }
                if current.steps.is_empty() {
                    *current = key;
                } else {
                    current.steps.extend(key.steps);
                }
            }
        }
        self.selected = target;
        if let Err(e) = crate::model::validate_key_in_profile(
            &self.config.keys()[target],
            &self.config,
            &self.config.profiles[active],
        ) {
            *self.config.keys_mut() = before;
            self.fail(format!("Affectation refusée : {e}"));
            return;
        }
        self.edited();

        self.log(format!(
            "Touche {:02} mise à jour. Ctrl+Z pour annuler.",
            target + 1
        ));
    }
    fn record_history(&mut self, before: Config, ctx: &egui::Context) {
        if self.replaying || serde_json::to_value(&before).ok() == serde_json::to_value(&self.config).ok() { return; }
        let field = ctx.memory(|m| m.focused());
        let grouped = field.is_some() && field == self.last_field && self.history_at.elapsed() < Duration::from_millis(800) && self.future.is_empty();
        if !grouped { self.history.push(before); if self.history.len() > 50 { self.history.remove(0); } }
        self.future.clear(); self.last_field = field; self.history_at = Instant::now();
    }
    fn migrate_focus_audio_now(&mut self) {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default();
        for name in ["config", "preferences"] {
            let source = self.path.with_file_name(format!("{name}.json"));
            if source.exists() {
                if let Err(e) = std::fs::copy(&source, self.path.with_file_name(format!("{name}.avant-migration-{stamp}.json"))) {
                    self.fail(format!("Conversion annulée : sauvegarde de {name}.json impossible ({e})."));
                    return;
                }
            }
        }
        self.history.push(self.config.clone());
        self.future.clear();
        let keys = self.config.migrate_focus_audio();
        let binds: usize = self.preferences.library.binds.iter_mut().map(|b| crate::model::migrate_focus_audio_key(&mut b.key)).sum();
        self.edited();
        self.save();
        if binds > 0 { self.save_preferences(); }
        self.log(format!("{keys} action(s) de touches et {binds} de Mes binds converties en blocs natifs. Sauvegardes : *.avant-migration-{stamp}.json (Ctrl+Z annule côté touches)."));
    }
    fn undo_edit(&mut self) {
        if let Some(previous) = self.history.pop() { self.future.push(self.config.clone()); self.config = previous; self.replaying = true; self.edited(); self.log("Modification annulée.".into()); }
    }
    fn redo_edit(&mut self) {
        if let Some(next) = self.future.pop() { self.history.push(self.config.clone()); self.config = next; self.replaying = true; self.edited(); self.log("Modification rétablie.".into()); }
    }
    fn copy_key(&mut self) { self.clipboard = Some(self.config.keys()[self.selected].clone()); self.log("Touche copiée.".into()); }
    fn paste_key(&mut self, target: usize) { if let Some(key) = self.clipboard.clone() { self.assign(DragItem::Bind(key), target); } }
    fn duplicate_key(&mut self, from: usize) {
        let Some(target) = ((from+1)..15).find(|&i| Self::empty_key(&self.config.keys()[i])) else { self.log("Aucune touche vide après celle-ci sur cette page.".into()); return; };
        self.assign(DragItem::Bind(self.config.keys()[from].clone()), target);
    }
    fn empty_key(key: &Key) -> bool { key.label.is_empty() && key.image.is_empty() && key.steps.is_empty() }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() { return; }
        let pressed = |key| ctx.input(|i| i.modifiers.ctrl && !i.modifiers.alt && !i.modifiers.shift && i.key_pressed(key));
        if pressed(egui::Key::Z) { self.undo_edit(); } else if pressed(egui::Key::Y) { self.redo_edit(); }
        else if self.show_workflow { }
        else if pressed(egui::Key::C) { self.copy_key(); } else if pressed(egui::Key::V) { self.paste_key(self.selected); }
        else if ctx.input(|i| !i.modifiers.ctrl && i.key_pressed(egui::Key::Delete)) { self.assign(DragItem::Bind(Key::default()), self.selected); }
    }
    fn import_elgato(&mut self) {
        self.gate.enabled = false;
        self.engine.stop();
        self.start_dialog(|| {
            let path = rfd::FileDialog::new()
                .add_filter("Profil ou action Stream Deck",
                    &["streamDeckProfile", "streamDeckAction", "json"]).pick_file()?;
            let result = crate::elgato::load(&path);
            Some(Box::new(move |app| match result {
                Ok(report) => app.import_report = Some(report),
                Err(e) => app.fail(format!("Import Stream Deck impossible : {e}")),
            }))
        });
    }
    fn apply_elgato(&mut self) {
        let Some(mut report) = self.import_report.take() else {
            return;
        };
        if self.config.profiles.len() >= 64 {
            self.fail("64 profils maximum.".into());
            self.import_report = Some(report);
            return;
        }
        let original = report.profile.name.clone();
        let mut suffix = 2;
        while self
            .config
            .profiles
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(&report.profile.name))
        {
            report.profile.name = format!("{original} ({suffix})");
            suffix += 1;
        }
        let result = (|| -> anyhow::Result<()> {
            if !report.icons.is_empty() {
                let dir = self.path.parent().unwrap().join("assets").join(format!(
                    "import-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_nanos()
                ));
                std::fs::create_dir_all(&dir)?;
                for (i, (page, key, bytes)) in report.icons.iter().enumerate() {
                    let path = dir.join(format!("icon-{i}.png"));
                    std::fs::write(&path, bytes)?;
                    report.profile.keys_at_mut(page).unwrap()[*key].image =
                        path.to_string_lossy().into_owned();
                }
            }
            let mut next = self.config.clone();
            next.profiles.push(report.profile.clone());
            next.active = next.profiles.len() - 1;
            next.reset_navigation();
            next.validate()?;
            store::save(&self.path, &next)?;
            self.config = next;
            self.selected = 0;
            self.last_field = None;
            self.gate.dirty = false;
            if self.wanted {
                self.upload(false);
            }
            Ok(())
        })();
        match result {
            Ok(()) => self.log(format!(
                "Profil importé : {} actions converties, {} avertissements. Touches en pause.",
                report.converted,
                report.warnings.len()
            )),
            Err(e) => {
                self.fail(format!("Import non enregistré : {e}"));
                self.import_report = Some(report);
            }
        }
    }
    fn import_binds(&mut self) {
        self.gate.enabled = false;
        self.engine.stop();
        self.start_dialog(|| {
            let path = rfd::FileDialog::new().add_filter("Pack DeckForge", &["json"]).pick_file()?;
            let pack = (|| -> anyhow::Result<Library> {
                let bytes = std::fs::read(path)?;
                anyhow::ensure!(bytes.len() <= 8_000_000, "Pack trop volumineux");
                Ok(serde_json::from_slice(&bytes)?)
            })();
            Some(Box::new(move |app| match pack {
                Ok(pack) => {
                    let result = (|| -> anyhow::Result<usize> {
                        let mut next = app.preferences.library.clone();
                        let count = next.merge(pack, &app.config)?;
                        let previous = std::mem::replace(&mut app.preferences.library, next);
                        if !app.save_preferences() {
                            app.preferences.library = previous;
                            anyhow::bail!("Pack non enregistré");
                        }
                        Ok(count)
                    })();
                    match result {
                        Ok(n) => app.log(format!("{n} binds importés. Les touches sont en pause.")),
                        Err(e) => app.fail(format!("Import refusé : {e}")),
                    }
                }
                Err(e) => app.fail(format!("Import refusé : {e}")),
            }))
        });
    }
    fn export_binds(&mut self) {
        self.gate.enabled = false;
        self.engine.stop();
        let library = self.preferences.library.clone();
        self.start_dialog(move || {
            let path = rfd::FileDialog::new().set_file_name("mes-binds-deckforge.json")
                .add_filter("Pack DeckForge", &["json"]).save_file()?;
            let result = serde_json::to_vec_pretty(&library)
                .map_err(anyhow::Error::from)
                .and_then(|b| std::fs::write(path, b).map_err(Into::into));
            Some(Box::new(move |app| match result {
                Ok(()) => app.log("Pack exporté. Les touches sont en pause.".into()),
                Err(e) => app.fail(format!("Export impossible : {e}")),
            }))
        });
    }
    fn catalog(&mut self, ctx: &egui::Context) {
        let p = theme::palette(ctx);
        egui::SidePanel::right("action_catalog")
            .exact_width(296.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(p.surface).inner_margin(16))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Bibliothèque d'actions").size(17.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        theme::pill(ui, "40 binds", BLUE);
                    });
                });
                ui.add_space(6.0);
                egui::Frame::new()
                    .fill(p.card)
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(8, 5))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (icon_r, _) =
                                ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
                            let c = icon_r.center() - Vec2::new(1.5, 1.5);
                            ui.painter()
                                .circle_stroke(c, 4.5, egui::Stroke::new(1.5_f32, p.muted));
                            ui.painter().line_segment(
                                [c + Vec2::new(3.2, 3.2), c + Vec2::new(6.5, 6.5)],
                                egui::Stroke::new(1.5_f32, p.muted),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut self.catalog_search)
                                    .frame(false)
                                    .desired_width(f32::INFINITY)
                                    .hint_text("Rechercher une action…"),
                            );
                        });
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    for (i, label) in ["Actions", "Magasin", "Mes binds", "Plugins"]
                        .iter()
                        .enumerate()
                    {
                        let (r, response) =
                            ui.allocate_exact_size(Vec2::new(62.0, 48.0), egui::Sense::click());
                        let selected = i == self.catalog_tab;
                        ui.painter().rect_filled(
                            r,
                            10.0,
                            if selected {
                                p.wash
                            } else if response.hovered() {
                                p.hover
                            } else {
                                p.surface
                            },
                        );
                        theme::symbol(
                            ui.painter(),
                            r.center_top() + Vec2::new(0.0, 15.0),
                            17.0,
                            [21, 20, 15, 15][i],
                            if selected { BLUE } else { p.muted },
                        );
                        ui.painter().text(
                            r.center_bottom() - Vec2::new(0.0, 6.0),
                            egui::Align2::CENTER_BOTTOM,
                            *label,
                            egui::FontId::proportional(11.0),
                            if selected { p.ink } else { p.muted },
                        );
                        if response.clicked() {
                            self.catalog_tab = i;
                            self.catalog_selection = None;
                            self.catalog_search.clear();
                        }
                    }
                });
                ui.add_space(6.0);
                let search = self.catalog_search.to_lowercase();
                if self.catalog_tab == 3 {
                    self.plugin_store(ui, &search);
                } else if self.catalog_tab == 0 {
                    ui.label(
                        RichText::new(
                            "Glisse une action sur une touche. Elle s’ajoute à la fin de sa macro.",
                        )
                        .small()
                        .color(p.muted),
                    );
                    let actions = crate::automation::NAMES;
                    egui::ScrollArea::vertical()
                        .id_salt("action_browser")
                        .show(ui, |ui| {
                            for (i, label) in actions.iter().enumerate() {
                                if !label.to_lowercase().contains(&search) {
                                    continue;
                                }
                                let item = if i == 12 {
                                    DragItem::NewFolder
                                } else {
                                    DragItem::Action(catalog::action(
                                        i,
                                        &self.config.profiles[self.config.active].name,
                                    ))
                                };
                                ui.push_id(i, |ui| {
                                    let (r, response) = ui.allocate_exact_size(
                                        Vec2::new(ui.available_width(), 50.0),
                                        egui::Sense::click_and_drag(),
                                    );
                                    ui.painter().rect_filled(
                                        r,
                                        12.0,
                                        if response.hovered() { p.hover } else { p.card },
                                    );
                                    let accent = match i % 5 {
                                        0 => BLUE,
                                        1 => theme::PURPLE,
                                        2 => GREEN,
                                        3 => theme::CYAN,
                                        _ => theme::WARNING,
                                    };
                                    let badge = egui::Rect::from_center_size(
                                        r.left_center() + Vec2::new(23.0, 0.0),
                                        Vec2::splat(32.0),
                                    );
                                    ui.painter()
                                        .rect_filled(badge, 8.0, accent.gamma_multiply(0.18));
                                    let kind = match &item {
                                        DragItem::Action(key) => crate::icons::kind(key),
                                        _ => 15,
                                    };
                                    theme::symbol(ui.painter(), badge.center(), 18.0, kind, accent);
                                    ui.painter().text(
                                        r.min + Vec2::new(47.0, 9.0),
                                        egui::Align2::LEFT_TOP,
                                        *label,
                                        egui::FontId::proportional(12.5),
                                        p.ink,
                                    );
                                    ui.painter().text(
                                        r.min + Vec2::new(47.0, 28.0),
                                        egui::Align2::LEFT_TOP,
                                        "Action · Glisser ou cliquer",
                                        egui::FontId::proportional(10.0),
                                        p.muted,
                                    );
                                    ui.painter().text(
                                        r.right_center() - Vec2::new(12.0, 0.0),
                                        egui::Align2::RIGHT_CENTER,
                                        "›",
                                        egui::FontId::proportional(16.0),
                                        p.muted,
                                    );
                                    self.drag_sources
                                        .push((response.rect.intersect(ui.clip_rect()), item.clone()));
                                    if response.clicked() {
                                        self.assign(item, self.selected);
                                    }
                                    response.on_hover_text(
                                        "Glisser sur une touche, ou cliquer pour ajouter à la touche sélectionnée",
                                    );
                                });
                            }
                            ui.add_space(14.0);
                            ui.label(RichText::new("Macro personnalisée").strong());
                            ui.label(
                                RichText::new("Combine plusieurs actions dans l’inspecteur du bas, puis enregistre le résultat dans Mes binds.")
                                    .small()
                                    .color(p.muted),
                            );
                        });
                } else {
                    if self.catalog_tab == 1 {
                        egui::ComboBox::from_id_salt("catalog_category")
                            .width(ui.available_width() - 8.0)
                            .selected_text(&self.catalog_category)
                            .show_ui(ui, |ui| {
                                for category in [
                                    "Tout",
                                    "Média",
                                    "Windows",
                                    "Édition",
                                    "Navigateur",
                                    "Sites",
                                    "Workflows",
                                ] {
                                    ui.selectable_value(
                                        &mut self.catalog_category,
                                        category.into(),
                                        category,
                                    );
                                }
                            });
                        ui.label(
                            RichText::new("Gratuit · hors ligne · modifiable")
                                .small()
                                .color(p.muted),
                        );
                    } else {
                        ui.horizontal(|ui| {
                            if ui.button("Enregistrer la touche").clicked() {
                                self.bind_name = self.config.keys()[self.selected].label.clone();
                                self.save_bind = true;
                            }
                            ui.menu_button("•••", |ui| {
                                if ui.button("Importer un pack…").clicked() {
                                    self.import_binds();
                                    ui.close_menu();
                                }
                                if ui.button("Exporter Mes binds…").clicked() {
                                    self.export_binds();
                                    ui.close_menu();
                                }
                            });
                        });
                    }
                    let items: Vec<(String, String, Key)> = if self.catalog_tab == 1 {
                        catalog::presets()
                            .into_iter()
                            .map(|v| (v.category.into(), v.description.into(), v.key))
                            .collect()
                    } else {
                        self.preferences
                            .library
                            .binds
                            .iter()
                            .map(|b| {
                                (
                                    "Personnel".into(),
                                    format!(
                                        "{} · {} étape(s). Les images restent liées à leur fichier local.",
                                        b.name,
                                        b.key.steps.len()
                                    ),
                                    {
                                        let mut k = b.key.clone();
                                        k.label = b.name.clone();
                                        k
                                    },
                                )
                            })
                            .collect()
                    };
                    let selected = self.catalog_selection.filter(|i| *i < items.len());
                    let detail_height = if selected.is_some() { 212.0 } else { 40.0 };
                    let mut found = 0;
                    egui::ScrollArea::vertical()
                        .id_salt("preset_list")
                        .max_height((ui.available_height() - detail_height).max(90.0))
                        .show(ui, |ui| {
                            for (i, (category, description, key)) in items.iter().enumerate() {
                                if self.catalog_tab == 1
                                    && self.catalog_category != "Tout"
                                    && self.catalog_category != *category
                                {
                                    continue;
                                }
                                if !format!("{} {} {}", key.label, category, description)
                                    .to_lowercase()
                                    .contains(&search)
                                {
                                    continue;
                                }
                                found += 1;
                                let response = ui
                                    .push_id(i, |ui| {
                                        let (r, response) = ui.allocate_exact_size(
                                            Vec2::new(ui.available_width(), 54.0),
                                            egui::Sense::click_and_drag(),
                                        );
                                        let fill = if selected == Some(i) {
                                            p.wash
                                        } else if response.hovered() {
                                            p.hover
                                        } else {
                                            p.card
                                        };
                                        ui.painter().rect_filled(r, 12.0, fill);
                                        if selected == Some(i) {
                                            ui.painter().rect_stroke(
                                                r,
                                                12.0,
                                                egui::Stroke::new(1.5_f32, BLUE),
                                                egui::StrokeKind::Inside,
                                            );
                                        }
                                        let color = Color32::from_rgb(
                                            key.color[0],
                                            key.color[1],
                                            key.color[2],
                                        );
                                        let badge = egui::Rect::from_center_size(
                                            r.left_center() + Vec2::new(24.0, 0.0),
                                            Vec2::splat(34.0),
                                        );
                                        ui.painter().rect_filled(
                                            badge,
                                            9.0,
                                            color.gamma_multiply(0.20),
                                        );
                                        theme::symbol(
                                            ui.painter(),
                                            badge.center(),
                                            18.0,
                                            crate::icons::kind(key),
                                            color,
                                        );
                                        let mut title =
                                            egui::text::LayoutJob::simple_singleline(
                                                key.label.clone(),
                                                egui::FontId::proportional(13.0),
                                                p.ink,
                                            );
                                        title.wrap.max_width = (r.width() - 72.0).max(20.0);
                                        title.wrap.max_rows = 1;
                                        ui.painter().galley(
                                            r.min + Vec2::new(49.0, 8.0),
                                            ui.painter().layout_job(title),
                                            p.ink,
                                        );
                                        ui.painter().text(
                                            r.min + Vec2::new(49.0, 30.0),
                                            egui::Align2::LEFT_TOP,
                                            format!(
                                                "{} · {} action(s)",
                                                category,
                                                key.steps.len()
                                            ),
                                            egui::FontId::proportional(10.0),
                                            p.muted,
                                        );
                                        ui.painter().text(
                                            r.right_center() - Vec2::new(12.0, 0.0),
                                            egui::Align2::RIGHT_CENTER,
                                            "›",
                                            egui::FontId::proportional(16.0),
                                            if selected == Some(i) { BLUE } else { p.muted },
                                        );
                                        response
                                    })
                                    .inner;
                                self.drag_sources.push((
                                    response.rect.intersect(ui.clip_rect()),
                                    DragItem::Bind(key.clone()),
                                ));
                                if response.clicked() {
                                    self.catalog_selection = Some(i);
                                }
                                response.on_hover_text(description);
                            }
                            if found == 0 {
                                ui.add_space(14.0);
                                ui.label(if self.catalog_tab == 2 && items.is_empty() {
                                    "Tes binds enregistrés apparaîtront ici."
                                } else {
                                    "Aucun résultat. Essaie un autre mot."
                                });
                            }
                        });
                    ui.separator();
                    if let Some(i) = self.catalog_selection.filter(|i| *i < items.len()) {
                        let (_, description, key) = &items[i];
                        egui::ScrollArea::vertical()
                            .id_salt("preset_details")
                            .max_height(195.0)
                            .show(ui, |ui| {
                                ui.label(RichText::new(&key.label).strong());
                                ui.label(RichText::new(description).small().color(p.muted));
                                ui.label(
                                    RichText::new(
                                        key.steps
                                            .iter()
                                            .map(step_summary)
                                            .collect::<Vec<_>>()
                                            .join(" → "),
                                    )
                                    .size(10.0)
                                    .color(p.muted),
                                );
                                if ui
                                    .add(theme::primary(
                                        RichText::new(format!(
                                            "Utiliser sur la touche {:02}",
                                            self.selected + 1
                                        ))
                                        .color(Color32::WHITE),
                                    ))
                                    .clicked()
                                {
                                    self.assign(DragItem::Bind(key.clone()), self.selected);
                                }
                                if self.catalog_tab == 2
                                    && ui.small_button("Supprimer de Mes binds").clicked()
                                {
                                    let previous = self.preferences.library.clone();
                                    self.preferences.library.binds.remove(i);
                                    if self.save_preferences() {
                                        self.catalog_selection = None;
                                    } else {
                                        self.preferences.library = previous;
                                    }
                                }
                            });
                    } else {
                        ui.label(
                            RichText::new(
                                "Glisse un bind sur une touche, ou clique pour voir ses actions.",
                            )
                            .small()
                            .color(p.muted),
                        );
                    }
                }
            });
    }
    fn footer(&mut self, ctx: &egui::Context) {
        let p = theme::palette(ctx);
        let validation = if !self.gate.invalid {
            self.config.binding_errors().into_iter().next()
        } else {
            None
        };
        let message = if let Some(e) = &validation {
            format!("Brouillon · {e} · Les autres touches restent disponibles.")
        } else if self.gate.dirty {
            "Sauvegarde automatique…".into()
        } else {
            self.notice.clone()
        };
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .inner_margin(egui::Margin::symmetric(20, 7)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&message).size(11.0).color(
                        if self.error || validation.is_some() {
                            Color32::from_rgb(233, 119, 109)
                        } else {
                            p.muted
                        },
                    ));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(if self.show_logs {
                                "Masquer le journal"
                            } else {
                                "Journal"
                            })
                            .clicked()
                        {
                            self.show_logs = !self.show_logs;
                        }
                        ui.label(
                            RichText::new("Arrêt : Ctrl + Alt + F12")
                                .size(10.0)
                                .color(p.muted),
                        );
                    });
                });
            });
        if self.show_logs {
            egui::TopBottomPanel::bottom("logs")
                .default_height(165.0)
                .height_range(100.0..=320.0)
                .resizable(true)
                .frame(egui::Frame::new().fill(p.surface).inner_margin(16))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Activité récente").strong());
                        if ui.small_button("Effacer").clicked() {
                            self.logs.clear();
                        }
                    });
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for s in &self.logs {
                                ui.label(RichText::new(s).monospace().size(11.0).color(p.muted));
                            }
                        });
                });
        }
    }
    fn editor(&mut self, ctx: &egui::Context) {
        let p = theme::palette(ctx);
        let running = self.engine.running.load(Ordering::SeqCst);
        egui::TopBottomPanel::bottom("key_inspector").default_height(if ctx.screen_rect().height()<800.0 {240.0}else{260.0}).height_range(240.0..=420.0).resizable(true).frame(egui::Frame::new().fill(p.surface).inner_margin(16)).show(ctx,|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Touche {:02}",self.selected+1)).size(16.0).strong());
                ui.selectable_value(&mut self.long_editor, false, "Appui court");
                ui.selectable_value(&mut self.long_editor, true, "Appui long");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                    if ui.add(theme::primary(RichText::new("Atelier de blocs").color(Color32::WHITE))).on_hover_text("Composer l’automatisation avec des blocs visuels").clicked(){self.show_workflow=true;}
                    if ui.button("Enregistrer dans Mes binds").on_hover_text("Conserver cette macro pour la réutiliser").clicked(){self.bind_name=self.config.keys()[self.selected].label.clone();self.save_bind=true;}
                    if ui.add_enabled(!self.config.keys()[self.selected].steps.is_empty(),egui::Button::new("Vider")).clicked(){self.assign(DragItem::Bind(Key::default()),self.selected);}
                    if ui.add_enabled(!running,egui::Button::new("Tester dans 3 s")).on_hover_text("Exécute la macro sur ton PC après 3 secondes, dans la fenêtre active.").clicked(){self.preview(false);}
                    if ui.add_enabled(!running,theme::primary(RichText::new("Simuler").color(Color32::WHITE))).on_hover_text("Affiche les étapes sans les exécuter.").clicked(){self.preview(true);}
                });
            });
            ui.separator();
            if let Some(shortcut) = extras::discord_shortcut(&self.config.keys()[self.selected]) {
                ui.group(|ui| {
                    ui.label("Discord : configure ce raccourci dans Discord → Paramètres → Raccourcis clavier");
                    ui.horizontal(|ui| {
                        ui.label(shortcut);
                        if ui.button("Copier").clicked() { ui.ctx().copy_text(shortcut.to_owned()); }
                    });
                    ui.label("État estimé : DeckForge ne lit pas l'état réel du micro ni du casque dans Discord.");
                });
            }
            let long_editor = self.long_editor;
            let active=self.config.active; let names=self.config.profiles.iter().map(|p|p.name.clone()).collect::<Vec<_>>();
            let pages=self.config.profiles[active].pages.clone();let selected=self.selected;let page_id=self.config.page.clone();let dialog_tx=self.dialog_tx.clone();let image_size=self.config.image_size;let key=&mut self.config.keys_mut()[selected];
            let mut removed_step=false; let mut changed=false; let mut add_kind=None; let mut record_step=None; let mut open_icons=false; let mut pick_audio=None; let mut open_workflow=false; let mut open_model=false;
            if let Some(binding)=key.plugin.clone(){
                ui.horizontal(|ui|{
                    ui.label(format!("Plugin : {} · {}",binding.plugin,binding.action));
                    ui.label(self.plugin_errors.get(&binding.plugin).map(String::as_str).unwrap_or("Actif"));
                    if self.plugins.as_ref().is_some_and(|m|m.inspector_path(selected).is_some())
                        && ui.button("Réglages du plugin").clicked(){if let Some(m)=&self.plugins {m.open_inspector(selected);}}
                    if ui.button("Retirer le plugin").clicked(){key.plugin=None;changed=true;}
                });
            }
            if self.flicker.as_ref().is_some_and(|session| session.active()) {self.diagnostic_inspector=Some(ui.max_rect());}
            let width=ui.available_width(); let body_height=ui.available_height();
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(Vec2::new(215.0,body_height),egui::Layout::top_down(egui::Align::Min),|ui| {
                    egui::ScrollArea::vertical().id_salt("key_appearance").auto_shrink([false, false]).max_height(body_height).show(ui,|ui| {
                        ui.label(RichText::new("Titre sur le D6").small().color(p.muted));
                        changed|=ui.add(egui::TextEdit::singleline(&mut key.label).desired_width(200.0).hint_text("Nom de la touche")).changed();
                        ui.horizontal(|ui| {
                            ui.label("Taille du titre");
                            changed |= ui.add(egui::Slider::new(&mut key.title_size, 10..=20).show_value(false)).changed();
                            ui.label(format!("{} px", key.title_size));
                        });
                        changed|=appearance::color(ui,key);
                        if crate::audio::single_switch(&key.steps).is_some(){if ui.button("Icônes par état →").on_hover_text("Choisir séparément les icônes des deux états audio").clicked(){open_icons=true;}}else{
                        let actions=appearance::icons(ui,!key.image.is_empty());
                        open_icons|=actions.gallery;
                        if actions.pick_file {
                            let profile=active;let page=page_id.clone();let index=selected;
                            let tx=self.dialog_tx.clone();
                            let assets=self.path.parent().unwrap().join("assets/mes-icones");
                            let size=image_size;
                            std::thread::spawn(move || {
                                let path=rfd::FileDialog::new().add_filter("Image ou vidéo", &["png","jpg","jpeg","svg","gif","mp4","webm","mov","mkv","avi"]).pick_file();
                                if let Some(path) = path {
                                    let video = crate::assets::is_video(&path);
                                    if video { let _ = tx.send(Some(Box::new(|app: &mut App| app.log("Conversion de la vidéo…".into())) as DialogEffect)); }
                                    let result = if video {
                                        crate::assets::import_video(&path, &assets, size).map_err(|e| e.to_string())
                                    } else if path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("gif")) {
                                        std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| {
                                            let oversized = image::image_dimensions(&path).map(|(w,h)| w > 512 || h > 512).unwrap_or(false);
                                            if oversized { let _ = tx.send(Some(Box::new(|app: &mut App| app.log("GIF trop grand : réduction automatique à 256 px.".into())) as DialogEffect)); }
                                            crate::assets::import_image(&bytes, &assets, path.file_stem().and_then(|s| s.to_str()).unwrap_or("gif")).map_err(|e| e.to_string())
                                        })
                                    } else { Ok(path) };
                                    let _=tx.send(Some(Box::new(move |app: &mut App| match result {
                                        Ok(path) => {
                                            if let Some(key)=app.config.profiles.get_mut(profile).and_then(|p|p.keys_at_mut(&page)).and_then(|keys|keys.get_mut(index)) {
                                                key.image=path.to_string_lossy().into_owned();app.edited();
                                                if video { app.log("Vidéo convertie en GIF animé.".into()); }
                                            }
                                        }
                                        Err(e) => app.fail(format!("Import image ou vidéo impossible : {e}")),
                                    }) as DialogEffect));
                                }
                            });
                        }
                        if actions.clear {key.image.clear();changed=true;}

                        }
                        ui.horizontal(|ui| {
                        ui.label("Taille de l’icône");
                            if ui.small_button("Défaut").on_hover_text("Remettre la taille de l’icône à 100 %").clicked() && key.icon_scale != 100 {
                                key.icon_scale = 100; changed = true;
                            }
                        });
                        changed |= ui.add(egui::Slider::new(&mut key.icon_scale, 20..=200).suffix(" %")).changed();
                        ui.horizontal(|ui|{ui.label("Répéter");changed|=ui.add(egui::DragValue::new(&mut key.repeat).range(1..=1000)).changed();ui.label("fois");});
                        ui.add_space(6.0);
                        ui.label(RichText::new("La simulation ne lance aucune action. Le test réel vise la fenêtre active.").size(10.0).color(p.muted));
                    });
                });
                ui.separator();
                ui.allocate_ui_with_layout(Vec2::new((width-240.0).max(100.0),body_height),egui::Layout::top_down(egui::Align::Min),|ui| {
                    if !long_editor && key.steps.len()==1 && matches!(key.steps[0],Step::AudioSwitch{..}) {
                        egui::ScrollArea::vertical().id_salt("compact_audio").max_height(body_height).show(ui,|ui|{let mut state=None;changed|=icon_ui::audio_options(ui,&mut key.steps[0],&self.audio,&mut state,&dialog_tx,&(active,page_id.clone(),selected,vec![0]));if let Some(state)=state{pick_audio=Some((vec![0],state));}});
                        return;
                    }
                    let steps = if long_editor { &mut key.long_steps } else { &mut key.steps };
                    if long_editor { ui.label(RichText::new("L’atelier de blocs modifie uniquement l’appui court. Pour un fichier, renseigne son chemin dans l’action.").small().color(p.muted)); }
                    ui.horizontal(|ui|{
                        ui.label(RichText::new(format!("Actions · {} étape(s)",steps.len())).strong());
                        ui.menu_button("+ Ajouter",|ui|{for (i,label) in crate::automation::NAMES.iter().enumerate(){if ui.add_enabled(steps.len()<256,egui::Button::new(*label)).clicked(){add_kind=Some(i);ui.close_menu();}}});
                    });
                    let mut remove=None;let mut movement=None;
                    egui::ScrollArea::vertical().id_salt("step_list").auto_shrink([false, false]).max_height((body_height-40.0).max(100.0)).show(ui,|ui| {
                        if steps.is_empty(){ui.add_space(12.0);ui.label(RichText::new("Glisse une action ou un bind sur cette touche.").strong());ui.label(RichText::new("La bibliothèque à droite propose des raccourcis prêts à l’emploi. Tu peux ensuite combiner et modifier leurs actions ici.").color(p.muted));}
                        if steps.is_empty(){ui.horizontal(|ui| {if ui.button("Ouvrir l’Atelier de blocs").clicked(){open_workflow=true;}if ui.button("Partir d’un modèle").clicked(){open_model=true;}});}
                        let count=steps.len();
                        for(i,step) in steps.iter_mut().enumerate(){ui.push_id(i,|ui|{
                            egui::Frame::new().fill(p.bg).corner_radius(6).inner_margin(10).show(ui,|ui| {
                                ui.set_max_width(ui.available_width());
                                ui.horizontal(|ui|{
                                    ui.label(RichText::new(format!("{:02}",i+1)).color(BLUE).size(11.0));
                                    ui.label(RichText::new(step.name()).strong().size(12.0));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                                        if ui.small_button("×").on_hover_text("Supprimer cette étape").clicked(){remove=Some(i);}
                                        if ui.add_enabled(i+1<count,egui::Button::new("↓").small()).clicked(){movement=Some((i,i+1));}
                                        if ui.add_enabled(i>0,egui::Button::new("↑").small()).clicked(){movement=Some((i,i-1));}
                                    });
                                });
                                if matches!(step,Step::AutoHotkey{..}) {ui.label(RichText::new("Pas besoin d’écrire du code : compose tes actions dans l’Atelier de blocs").small().color(p.muted));if ui.button("Ouvrir l’Atelier de blocs").clicked(){open_workflow=true;}}
                                let mut record=false;let mut audio_icon=None;changed|=edit_step(ui,step,&names,&pages,&mut record,&self.audio,&mut audio_icon,&dialog_tx,&(active,page_id.clone(),selected,vec![i]));if let Some(state)=audio_icon{pick_audio=Some((vec![i],state));}if record && !long_editor {record_step=Some(i);}
                            });
                        });}
                    });
                    if let Some(i)=remove{steps.remove(i);changed=true;removed_step=true;}if let Some((a,b))=movement{steps.swap(a,b);changed=true;}
                });
            });
            if open_workflow {self.show_workflow=true;}if open_model {self.show_workflow=true;}
            if let Some(step)=record_step {self.recording=Some((active,self.config.page.clone(),self.selected,vec![step],self.gate.enabled));self.recording_error.clear();self.gate.enabled=false;self.engine.stop();self.refresh_hotkeys();}
            if changed {self.edited(); if removed_step {self.log("Étape retirée — Ctrl+Z pour annuler".into());}}
            if open_icons{self.open_icon_gallery(None);}if let Some(target)=pick_audio{self.open_icon_gallery(Some(target));}
            if let Some(i)=add_kind {if i==12{self.create_page(true);}else if long_editor {
                let action = catalog::action(i,&names[active]);
                self.config.keys_mut()[selected].long_steps.extend(action.steps);
                self.edited();
            }else{self.assign(DragItem::Action(catalog::action(i,&names[active])),self.selected);}}

        });
    }

    fn deck(&mut self, ctx: &egui::Context) {
        let p = theme::palette(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.bg).inner_margin(18))
            .show(ctx, |ui| {
                if self.flicker.as_ref().is_some_and(|session| session.active()) {
                    self.diagnostic_deck = Some(ui.max_rect());
                }
                theme::gradient(
                    ui.painter(),
                    ui.max_rect(),
                    p.bg,
                    if self.preferences.dark {
                        Color32::from_rgb(29, 34, 49)
                    } else {
                        Color32::from_rgb(225, 231, 248)
                    },
                );
                let in_folder = !self.config.page.is_empty()
                    && self.config.profiles[self.config.active]
                        .pages
                        .iter()
                        .any(|page| page.id == self.config.page && page.folder);
                let non_folder_pages: Vec<_> = self.config.profiles[self.config.active]
                    .pages
                    .iter()
                    .filter(|page| !page.folder)
                    .collect();
                let total_pages = 1 + non_folder_pages.len();
                let current_page_num = if self.config.page.is_empty() {
                    1
                } else {
                    non_folder_pages
                        .iter()
                        .position(|page| page.id == self.config.page)
                        .map(|idx| idx + 2)
                        .unwrap_or(1)
                };
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&self.config.profiles[self.config.active].name)
                            .size(18.0)
                            .strong(),
                    );
                    if self.auto_active {
                        ui.label(
                            RichText::new(format!(
                                "Auto : {}",
                                self.config.profiles[self.config.active].name
                            ))
                            .small()
                            .color(p.muted),
                        );
                    }
                    let center_x = ui.max_rect().center().x;
                    let pad = (center_x - 80.0 - ui.cursor().min.x).max(12.0);
                    ui.add_space(pad);
                    if in_folder {
                        if ui.button("← Retour").clicked() {
                            self.navigate(Step::Back);
                        }
                        ui.label(RichText::new("DOSSIER").small().color(p.muted));
                        let name = self.config.profiles[self.config.active]
                            .pages
                            .iter()
                            .find(|page| page.id == self.config.page)
                            .map(|page| page.name.as_str())
                            .unwrap_or_default();
                        ui.label(RichText::new(name).strong());
                    } else {
                        if ui
                            .small_button("‹")
                            .on_hover_text("Page précédente")
                            .clicked()
                        {
                            self.navigate(Step::PagePrevious);
                        }
                        ui.label(
                            RichText::new(format!(
                                "Page {} / {}",
                                current_page_num, total_pages
                            ))
                            .size(13.0)
                            .strong(),
                        );
                        if ui
                            .small_button("›")
                            .on_hover_text("Page suivante")
                            .clicked()
                        {
                            self.navigate(Step::PageNext);
                        }
                    }
                    if !self.config.page.is_empty() {
                        ui.menu_button("Renommer", |ui| {
                            if let Some(page) = self.config.profiles[self.config.active]
                                .pages
                                .iter_mut()
                                .find(|page| page.id == self.config.page)
                            {
                                if ui.text_edit_singleline(&mut page.name).changed() {
                                    self.edited();
                                }
                            }
                        });
                    }
                });
                if !self.config.trail.is_empty() {
                    let profile = &self.config.profiles[self.config.active];
                    let path = self
                        .config
                        .trail
                        .iter()
                        .chain(std::iter::once(&self.config.page))
                        .map(|id| {
                            profile
                                .pages
                                .iter()
                                .find(|p| &p.id == id)
                                .map(|p| p.name.as_str())
                                .unwrap_or("Page 1")
                        })
                        .collect::<Vec<_>>()
                        .join("  ›  ");
                    ui.label(RichText::new(path).small().color(p.muted));
                }
                ui.add_space(7.0);
                if ctx.screen_rect().width() >= 1200.0 {
                    ui.label(
                        RichText::new(
                            "Glisse un bind sur une touche, puis personnalise ses actions dans l’inspecteur.",
                        )
                        .small()
                        .color(p.muted),
                    );
                }
                {
                    // Le pavé suit la taille de la fenêtre : il occupe tout l'espace disponible, sans barre de défilement.
                    // Largeur : 5 touches + 4 espacements (56) + marges (48). Hauteur : 3 touches + 2 espacements (28) + marges (68) + 4 px de sécurité.
                    let width = ((ui.available_width() - 104.0) / 5.0)
                        .min((ui.available_height() - 100.0) / 3.0)
                        .floor()
                        .clamp(40.0, 320.0);
                    if self.flicker.as_ref().is_some_and(|session| session.active()) {
                        self.diagnostic_key_width = width;
                    }
                    let total = width * 5.0 + 56.0 + 48.0;
                    ui.horizontal(|ui| {
                        ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                        let body = egui::Rect::from_min_size(
                            ui.cursor().min,
                            Vec2::new(total, width * 3.0 + 28.0 + 68.0),
                        );
                        theme::device_shell(ui.painter(), body);
                        let device = egui::Frame::new()
                            .fill(Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE)
                            .shadow(egui::epaint::Shadow {
                                offset: [0, 12],
                                blur: 24,
                                spread: 0,
                                color: Color32::from_black_alpha(75),
                            })
                            .corner_radius(20)
                            .inner_margin(egui::Margin {
                                left: 24,
                                right: 24,
                                top: 24,
                                bottom: 44,
                            })
                            .show(ui, |ui| {
                                egui::Grid::new("deck_grid_v3")
                                    .spacing([14.0, 14.0])
                                    .show(ui, |ui| {
                                        for i in 0..15 {
                                            let mut key = self.presented_key(i);
                                            let face = self.plugin_face(i).cloned();
                                            if let Some(face) = &face {
                                                key = self.plugin_render_key(i, key, face);
                                            }
                                            let selected = i == self.selected;
                                            let flashing = self.last_key.is_some_and(|(idx, t)| {
                                                idx == i && t.elapsed() < Duration::from_millis(350)
                                            });
                                            let (r, response) = ui.allocate_exact_size(
                                                Vec2::splat(width),
                                                egui::Sense::click_and_drag(),
                                            );
                                            let hovering = self.drag.moving()
                                                && self
                                                    .drag
                                                    .position
                                                    .is_some_and(|pos| r.contains(pos));
                                            theme::key_face(
                                                ui.painter(),
                                                r,
                                                selected || hovering,
                                                flashing,
                                            );
                                            let lcd = r.shrink(5.0);
                                            match self.key_previews.texture(
                                                ctx,
                                                i,
                                                &key,
                                                self.config.image_size,
                                                face.as_ref(),
                                            ) {
                                                Ok(texture) => {
                                                    ui.painter().image(
                                                        texture.id(),
                                                        lcd,
                                                        egui::Rect::from_min_max(
                                                            egui::Pos2::ZERO,
                                                            egui::pos2(1.0, 1.0),
                                                        ),
                                                        Color32::WHITE,
                                                    );
                                                }
                                                Err(error) => {
                                                    ui.painter().text(
                                                        lcd.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        "Aperçu indisponible",
                                                        egui::FontId::proportional(9.0),
                                                        p.muted,
                                                    );
                                                    ui.painter().text(
                                                        lcd.left_top(),
                                                        egui::Align2::LEFT_TOP,
                                                        "!",
                                                        egui::FontId::proportional(12.0),
                                                        Color32::LIGHT_RED,
                                                    );
                                                    let _ = error;
                                                }
                                            }
                                            // Editing hints never alter the image sent to the LCD.
                                            if !key.long_steps.is_empty() {
                                                ui.painter().rect_filled(
                                                    egui::Rect::from_min_size(
                                                        lcd.right_top() - Vec2::new(17.0, 0.0),
                                                        Vec2::new(17.0, 16.0),
                                                    ),
                                                    3.0,
                                                    p.surface,
                                                );
                                                ui.painter().text(
                                                    lcd.right_top() + Vec2::new(-8.0, 8.0),
                                                    egui::Align2::CENTER_CENTER,
                                                    "L",
                                                    egui::FontId::proportional(11.0),
                                                    p.ink,
                                                );
                                            }
                                            if key.steps.is_empty()
                                                && key.image.is_empty()
                                                && key.label.is_empty()
                                            {
                                                theme::symbol(
                                                    ui.painter(),
                                                    r.center(),
                                                    width * 0.22,
                                                    8,
                                                    p.muted,
                                                );
                                            }
                                            if crate::model::validate_key_in_profile(
                                                &self.config.keys()[i],
                                                &self.config,
                                                &self.config.profiles[self.config.active],
                                            )
                                            .is_err()
                                            {
                                                ui.painter().circle_filled(
                                                    r.left_top() + Vec2::new(10.0, 10.0),
                                                    4.0,
                                                    Color32::from_rgb(242, 126, 106),
                                                );
                                            }
                                            if width >= 70.0 {
                                                ui.painter().text(
                                                    r.right_bottom() + Vec2::new(0.0, 3.0),
                                                    egui::Align2::RIGHT_TOP,
                                                    format!("{:02}", i + 1),
                                                    egui::FontId::proportional(8.0),
                                                    p.muted,
                                                );
                                            }
                                            self.drag_sources.push((
                                                r.intersect(ui.clip_rect()),
                                                DragItem::Position(
                                                    self.config.active,
                                                    self.config.page.clone(),
                                                    i,
                                                ),
                                            ));
                                            self.drop_targets
                                                .push((r.intersect(ui.clip_rect()), i));
                                            if response.clicked() {
                                                self.selected = i;
                                                self.retarget_gallery();
                                            }
                                            response.context_menu(|ui| {
                                                if ui.button("Copier").clicked() {
                                                    self.selected = i;
                                                    self.copy_key();
                                                    ui.close_menu();
                                                }
                                                if ui
                                                    .add_enabled(
                                                        self.clipboard.is_some(),
                                                        egui::Button::new("Coller"),
                                                    )
                                                    .clicked()
                                                {
                                                    self.paste_key(i);
                                                    ui.close_menu();
                                                }
                                                if ui
                                                    .button("Dupliquer vers la prochaine touche vide")
                                                    .clicked()
                                                {
                                                    self.duplicate_key(i);
                                                    ui.close_menu();
                                                }
                                                if ui.button("Vider").clicked() {
                                                    self.assign(DragItem::Bind(Key::default()), i);
                                                    ui.close_menu();
                                                }
                                            });
                                            if response.double_clicked() {
                                                if let Some(step @ Step::Folder { .. }) =
                                                    key.steps.first()
                                                {
                                                    self.navigate(step.clone());
                                                }
                                            }

                                            response.on_hover_text(format!(
                                                "Touche {:02} · {}\nGlisse depuis la bibliothèque pour affecter un bind. Glisse une touche sur une autre pour les échanger.",
                                                i + 1,
                                                key.label
                                            ));
                                            if i % 5 == 4 {
                                                ui.end_row();
                                            }
                                        }
                                    });
                            });
                        let edge = device.response.rect;
                        ui.painter().line_segment(
                            [
                                edge.left_top() + Vec2::new(24.0, 2.0),
                                edge.right_top() + Vec2::new(-24.0, 2.0),
                            ],
                            egui::Stroke::new(1.0_f32, Color32::from_white_alpha(35)),
                        );
                        ui.painter().text(
                            edge.center_bottom() - Vec2::new(0.0, 13.0),
                            egui::Align2::CENTER_CENTER,
                            "D E C K F O R G E    /    D 6",
                            egui::FontId::proportional(8.0),
                            Color32::from_rgb(118, 127, 146),
                        );
                    });
                }
            });
    }
    fn windows(&mut self, ctx: &egui::Context) {
        let p = theme::palette(ctx);
        if let Some((_, _, _, _, enabled)) = self.recording.clone() {
            let mut open = true;
            let mut cancel = false;
            egui::Window::new("Enregistrer un raccourci").open(&mut open).collapsible(false).resizable(false).default_width(410.0).show(ctx,|ui|{
                ui.label(RichText::new("Appuie sur la combinaison souhaitée").size(20.0).strong());
                ui.label("Exemple : maintiens Ctrl + Maj, puis appuie sur S.");
                ui.label(RichText::new("Seules les touches reçues par cette fenêtre sont capturées. Aucun enregistrement en arrière-plan. Les raccourcis réservés à Windows peuvent quitter la fenêtre.").small().color(p.muted));
                if !self.recording_error.is_empty(){ui.colored_label(Color32::LIGHT_RED,&self.recording_error);}
                if ui.button("Annuler l’enregistrement").clicked(){cancel=true;}
            });
            if !open || cancel {
                self.recording = None;
                self.gate.enabled = enabled;
                self.refresh_hotkeys();
            }
        }
        if self.page_manager {
            let mut open = true;
            let mut target = None;
            let mut delete = None;
            let mut changed = false;
            egui::Window::new("Pages et dossiers")
                .open(&mut open)
                .default_width(480.0)
                .show(ctx, |ui| {
                    ui.label("Renomme ou ouvre les pages et dossiers de ce profil.");
                    if ui.button("Page 1").clicked() {
                        target = Some(Step::Page { id: String::new() });
                    }
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            for page in &mut self.config.profiles[self.config.active].pages {
                                ui.push_id(&page.id, |ui| {
                                    ui.horizontal(|ui| {
                                        let (r, _) = ui.allocate_exact_size(
                                            Vec2::splat(20.0),
                                            egui::Sense::hover(),
                                        );
                                        theme::symbol(
                                            ui.painter(),
                                            r.center(),
                                            17.0,
                                            if page.folder { 15 } else { 17 },
                                            BLUE,
                                        );
                                        changed |=
                                            ui.text_edit_singleline(&mut page.name).changed();
                                        if ui.small_button("Ouvrir").clicked() {
                                            target = Some(if page.folder {
                                                Step::Folder {
                                                    id: page.id.clone(),
                                                }
                                            } else {
                                                Step::Page {
                                                    id: page.id.clone(),
                                                }
                                            });
                                        }
                                        if ui.small_button("Supprimer…").clicked() {
                                            delete = Some(page.id.clone());
                                        }
                                    });
                                });
                            }
                        });
                });
            self.page_manager = open;
            if changed {
                self.edited();
            }
            if let Some(step) = target {
                self.page_manager = false;
                self.navigate(step);
            }
            if let Some(id) = delete {
                self.delete_page = Some(id);
            }
        }
        if let Some(id) = self.delete_page.clone() {
            let mut open = true;
            let mut remove = false;
            egui::Window::new("Supprimer cette page ?").open(&mut open).collapsible(false).resizable(false).default_width(380.0).show(ctx,|ui|{
                ui.label("Son contenu et les touches qui ouvrent cette page seront retirés du profil. Les autres pages seront conservées.");
                if ui.button(RichText::new("Supprimer la page").color(Color32::LIGHT_RED)).clicked(){remove=true;}
            });
            if remove {
                self.remove_page(&id);
                self.delete_page = None;
            } else if !open {
                self.delete_page = None;
            }
        }
        if self.import_report.is_some() {
            let mut open = true;
            let mut install = false;
            if let Some(report) = &self.import_report {
                egui::Window::new("Import Stream Deck — aperçu").open(&mut open).collapsible(false).default_width(560.0).show(ctx,|ui| {
                    ui.label(RichText::new(&report.profile.name).size(22.0).strong());
                    ui.label(format!("{} actions converties · {} pages/dossiers · {} icônes",report.converted,report.profile.pages.len()+1,report.icons.len()));
                    ui.label(RichText::new("Un nouveau profil sera ajouté. Les actions non reconnues restent vides ; aucun plugin Elgato n’est installé. Les touches restent en pause après l’import.").small().color(p.muted));
                    egui::ScrollArea::vertical().max_height(340.0).show(ui,|ui| {
                        for warning in &report.warnings{ui.colored_label(Color32::from_rgb(222,165,77),warning);}
                        ui.separator();for key in report.profile.keys.iter().filter(|k|!k.label.is_empty()){ui.label(format!("{} · {} action(s)",key.label,key.steps.len()));}
                    });
                    if ui.add(theme::primary(RichText::new("Importer ce profil").color(Color32::WHITE))).clicked(){install=true;}
                });
            }
            if install {
                self.apply_elgato();
            } else if !open {
                self.import_report = None;
            }
        }
        if self.save_bind {
            let mut open = true;
            let mut save = false;
            egui::Window::new("Enregistrer dans Mes binds")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(380.0)
                .show(ctx, |ui| {
                    ui.label("Conserve une copie de cette touche et de toutes ses actions.");
                    ui.label(
                        RichText::new("Tu pourras ensuite la glisser sur n’importe quelle touche.")
                            .small()
                            .color(p.muted),
                    );
                    ui.text_edit_singleline(&mut self.bind_name);
                    let key = &self.config.keys()[self.selected];
                    ui.label(format!(
                        "{} action(s) · répétée(s) {} fois",
                        key.steps.len(),
                        key.repeat
                    ));
                    if ui
                        .add_enabled(
                            !key.steps.is_empty() && !self.bind_name.trim().is_empty(),
                            theme::primary(
                                RichText::new("Enregistrer le bind").color(Color32::WHITE),
                            ),
                        )
                        .clicked()
                    {
                        save = true;
                    }
                });
            if save {
                let key = self.config.keys()[self.selected].clone();
                let previous = self.preferences.library.clone();
                match self
                    .preferences
                    .library
                    .insert(&self.bind_name, key, &self.config)
                {
                    Ok(()) => {
                        if self.save_preferences() {
                            open = false;
                            self.catalog_tab = 2;
                            self.catalog_selection = Some(self.preferences.library.binds.len() - 1);
                            self.catalog_search.clear();
                            self.log("Bind enregistré dans Mes binds.".into());
                        } else {
                            self.preferences.library = previous;
                        }
                    }
                    Err(e) => self.fail(format!("Bind non enregistré : {e}")),
                }
            }
            self.save_bind = open;
        }
        if let Some(rx) = &self.update_rx {
            if let Ok(message) = rx.try_recv() {
                self.update_result = Some(message);
                self.update_rx = None;
            }
        }
        if self.show_about {
            let mut open = true;
            egui::Window::new("À propos de DeckForge")
                .open(&mut open).collapsible(false).resizable(false).default_width(370.0)
                .show(ctx, |ui| {
                    ui.label(RichText::new("DeckForge").size(20.0).strong());
                    let legacy_audio = {
                        let mut config = self.config.clone();
                        let mut binds = self.preferences.library.binds.clone();
                        config.migrate_focus_audio() + binds.iter_mut().map(|b| crate::model::migrate_focus_audio_key(&mut b.key)).sum::<usize>()
                    };
                    if legacy_audio > 0 {
                        ui.colored_label(egui::Color32::from_rgb(240, 180, 60), format!("{legacy_audio} action(s) utilisent l’ancien module FocusAudio.exe."));
                        if ui.button("Convertir en blocs natifs « Volume d’une application »").on_hover_text("Sauvegarde config.json et preferences.json à côté avant la conversion. Titres, icônes et couleurs restent inchangés.").clicked() {
                            self.migrate_focus_audio_now();
                        }
                        ui.separator();
                    }
                    ui.label(format!("Version : {}", env!("CARGO_PKG_VERSION")));
                    ui.label(format!("Compilation : {} ({})", about::HASH, about::DATE));
                    if ui.button("Copier ces informations").clicked() {
                        ui.ctx().copy_text(about::information());
                    }
                    if ui.button("Ouvrir la page GitHub").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab("https://github.com/HatiFlash/DeckForge"));
                    }
                    if ui.add_enabled(self.update_rx.is_none(), egui::Button::new("Vérifier les mises à jour")).clicked() {
                        self.update_result = Some("Vérification en cours…".into());
                        let (tx, rx) = mpsc::channel();
                        self.update_rx = Some(rx);
                        let context = ctx.clone();
                        std::thread::spawn(move || {
                            let _ = tx.send(about::check());
                            context.request_repaint();
                        });
                    }
                    if let Some(message) = &self.update_result { ui.label(message); }
                });
            self.show_about = open;
        }
        if self.show_apps {
            let mut open = true;
            egui::Window::new("Applications associées").open(&mut open).show(ctx, |ui| {
                ui.label("Noms d'exécutables (ex. obs64.exe) associés à ce profil.");
                let apps = &mut self.config.profiles[self.config.active].apps;
                let mut changed = false;
                let mut remove = None;
                for (i, name) in apps.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        changed |= ui.text_edit_singleline(name).changed();
                        if ui.small_button("Supprimer").clicked() { remove = Some(i); }
                    });
                }
                if let Some(i) = remove { apps.remove(i); changed = true; }
                if ui.button("Ajouter une application").clicked() { apps.push(String::new()); changed = true; }
                if ui.button("Ajouter l'application au premier plan dans 3 s").clicked() {
                    let profile = self.config.active;
                    self.start_dialog(move || {
                        std::thread::sleep(Duration::from_secs(3));
                        let exe = crate::desktop::foreground_exe()?;
                        Some(Box::new(move |app: &mut App| {
                            if let Some(p) = app.config.profiles.get_mut(profile) {
                                if !p.apps.iter().any(|name| name.eq_ignore_ascii_case(&exe)) { p.apps.push(exe); app.edited(); }
                            }
                        }))
                    });
                }
                if changed { self.edited(); }
            });
            self.show_apps = open;
        }
        if self.settings {
            let mut open = true;
            egui::Window::new("Réglages du D6")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(430.0)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Affichage").size(18.0).strong());
                    ui.add_space(8.0);
                    if ui
                        .add(
                            egui::Slider::new(&mut self.config.brightness, 0..=100)
                                .text("Luminosité"),
                        )
                        .changed()
                    {
                        self.edited();
                    }
                    if ui.add(egui::Slider::new(&mut self.config.sleep_after_min, 0..=240).text("Veille après (min, 0 = jamais)")).changed() { self.edited(); }
                    if ui.add(egui::Slider::new(&mut self.config.sleep_brightness, 0..=100).text("Luminosité en veille")).changed() { self.edited(); }
                    if ui.checkbox(&mut self.config.auto_profile, "Changer de profil selon l'application").changed() { self.edited(); self.update_auto_profile(); }
                    ui.add_space(8.0);
                    if ui.add(egui::Slider::new(&mut self.config.long_press_ms, 250..=2000).text("Appui long (ms)")).changed() { self.edited(); }
                    ui.label(RichText::new("Une touche avec appui long attend ce délai ; sans relâchement détecté, elle lance l’action longue.").small().color(p.muted));
                    if ui.checkbox(&mut self.config.d6_animations, "Animations sur le D6").changed() { self.edited(); }
                    if ui.add(egui::Slider::new(&mut self.config.animation_fps, 1..=15).text("Images/s max")).changed() { self.edited(); }
                    ui.label(RichText::new("Animation des GIF : protocole à vérifier sur le matériel D6.").small().color(p.muted));
                    ui.add_space(16.0);
                    ui.separator();
                    ui.label(RichText::new("Diagnostic du clignotement").strong());
                    let running=self.flicker.as_ref().is_some_and(|session|session.active());
                    if ui.button(if running {"Arrêter le diagnostic"} else {"Démarrer le diagnostic (90 s max)"}).clicked() {
                        if running { if let Some(session)=&self.flicker {session.stop();} }
                        else { match crate::flicker_diagnostic::Session::start(&self.path) {
                            Ok(session)=> {self.flicker_path=Some(session.path.clone()); self.flicker=Some(session);}
                            Err(e)=> self.fail(format!("Diagnostic indisponible : {e}")),
                        }}
                    }
                    ui.label("Capture locale sans réseau, limitée à 90 s et 6 Mo. Survole l'aperçu puis les réglages avant d'arrêter.");
                    if let Some(path)=&self.flicker_path {
                        let display=path.to_string_lossy().into_owned();
                        ui.label(format!("Fichier : {display}"));
                        if ui.button("Copier le chemin du diagnostic").clicked() {ui.ctx().copy_text(display);}
                    }
                    if let Some(session)=&self.flicker { ui.label(format!("{} · pertes : {}",if session.active(){"En cours"}else{"Arrêté"},session.dropped())); }
                    ui.add_space(12.0);
                    ui.label(RichText::new("Diagnostic des touches").strong());
                    if ui
                        .checkbox(
                            &mut self.gate.inspect,
                            "Sélectionner les touches sans exécuter d’action",
                        )
                        .changed()
                    {
                        self.engine.stop();
                    }
                    ui.label(
                        RichText::new("Le mode actif est indiqué dans la barre supérieure.")
                            .small()
                            .color(p.muted),
                    );
                    ui.add_space(12.0);
                    egui::CollapsingHeader::new("Compatibilité avancée").show(ui, |ui| {
                        if ui
                            .checkbox(
                                &mut self.config.flip_input_rows,
                                "Inverser les lignes des appuis",
                            )
                            .changed()
                        {
                            self.edited();
                        }
                        ui.horizontal(|ui| {
                            ui.label("Images");
                            if ui
                                .selectable_value(&mut self.config.image_size, 126, "126 px")
                                .changed()
                                || ui
                                    .selectable_value(&mut self.config.image_size, 112, "112 px")
                                    .changed()
                            {
                                self.edited();
                            }
                        });
                        ui.label(
                            RichText::new(
                                "Ton D6 a été validé en 126 px, sans inversion des appuis.",
                            )
                            .small()
                            .color(p.muted),
                        );
                    });
                });
            self.settings = open;
        }
        if self.rename {
            let mut open = true;
            egui::Window::new("Ce profil")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    let old = self.config.profiles[self.config.active].name.clone();
                    ui.label("Nom du profil");
                    if ui
                        .text_edit_singleline(&mut self.config.profiles[self.config.active].name)
                        .changed()
                    {
                        let new = self.config.profiles[self.config.active].name.clone();
                        for p in &mut self.config.profiles {
                            for k in std::iter::once(&mut p.keys)
                                .chain(p.pages.iter_mut().map(|page| &mut page.keys))
                                .flatten()
                            {
                                for s in &mut k.steps {
                                    if let Step::Profile { name } = s {
                                        if *name == old {
                                            *name = new.clone();
                                        }
                                    }
                                }
                            }
                        }
                        self.edited();
                    }
                    if ui
                        .add_enabled(
                            self.config.profiles.len() < 64,
                            egui::Button::new("Dupliquer ce profil"),
                        )
                        .clicked()
                    {
                        self.add_profile(true);
                    }
                });
            self.rename = open;
        }
    }
    fn frame(&mut self, ctx: &egui::Context) {
        let history_before = self.config.clone();
        self.replaying = false;
        self.dialog_results();
        self.record_trigger(ctx);
        self.record_events(ctx);
        self.events();
        self.plugin_events();
        self.housekeeping();
        self.assistant_events();
        let modal = self.show_workflow
            || self.show_assets
            || self.show_desktop_settings
            || self.page_manager
            || self.delete_page.is_some()
            || self.recording.is_some()
            || self.import_report.is_some()
            || self.settings
            || self.show_about
            || self.rename
            || self.save_bind
            || self.assistant_settings
            || !self.proposals.pending.is_empty()
            || ctx.memory(|m| m.any_popup_open());
        if modal {
            self.drag.cancel();
        } else {
            let events = ctx.input(|i| i.events.clone());
            if let Some((item, target)) =
                self.drag
                    .update(&events, &self.drag_sources, &self.drop_targets)
            {
                self.assign(item, target);
            }
        }
        self.drag_sources.clear();
        self.drop_targets.clear();
        self.shortcuts(ctx);
        self.frames += 1;
        self.key_previews.begin_frame();
        self.screenshot(ctx);
        self.header(ctx);
        self.footer(ctx);
        if !self.show_workflow {
            self.catalog(ctx);
            // Inspector first for now: full width under the navigation until its tabbed redesign.
            self.editor(ctx);
            self.sidebar(ctx);
            self.deck(ctx);
        } else {
            self.workflow(ctx);
        }
        if self.drag.moving() {
            if let (Some((item, _)), Some(pos)) = (&self.drag.active, self.drag.position) {
                let label = match item {
                    DragItem::Bind(k) | DragItem::Action(k) => k.label.clone(),
                    DragItem::Position(_, _, i) => format!("Touche {:02}", i + 1),
                    DragItem::NewFolder => "Nouveau dossier".into(),
                };
                egui::Area::new(egui::Id::new("drag_preview"))
                    .order(egui::Order::Tooltip)
                    .interactable(false)
                    .fixed_pos(pos + Vec2::new(14.0, 14.0))
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.label(label);
                        });
                    });
            }
        }
        self.extras(ctx);
        self.windows(ctx);
        self.assistant_window(ctx);
        self.key_previews.end_frame(ctx);
        self.record_history(history_before, ctx);
        if self.gate.dirty
            && !self.gate.invalid
            && self.last_edit.elapsed() > Duration::from_millis(800)
            && self.config.validate_structure().is_ok()
        {
            self.save();
        }
    }
    fn diagnostic_sample(&mut self, ctx:&egui::Context, start:Instant) {
        if self.flicker.as_ref().is_some_and(|s|s.active()) {
            let screen=ctx.screen_rect();
            let (pointer,focused,ppp)=ctx.input(|i|(i.pointer.hover_pos(),i.viewport().focused.unwrap_or(false),i.pixels_per_point()));
            let zone=match pointer {Some(pos) if self.diagnostic_inspector.is_some_and(|r|r.contains(pos))=>"inspector",Some(pos) if self.diagnostic_deck.is_some_and(|r|r.contains(pos))=>"preview",Some(_)=>"other",None=>"outside"};
            let rect=|r:Option<egui::Rect>|r.map(|r|[r.min.x,r.min.y,r.width(),r.height()]);
            let stats=self.key_previews.diagnostic_stats();
            let details=serde_json::json!({"mouse":pointer.map(|p|[p.x-screen.min.x,p.y-screen.min.y]),"viewport":[screen.width(),screen.height()],"pixels_per_point":ppp,"focused":focused,"preview_rect":rect(self.diagnostic_deck),"inspector_rect":rect(self.diagnostic_inspector),"key_width":self.diagnostic_key_width,"cache_hits":stats.0,"cache_misses":stats.1,"texture_recreates":stats.2,"animation_frames":stats.3,"repaint_animation_ms":stats.4});
            if let Some(session)=&mut self.flicker {session.frame(zone,details,start.elapsed().as_secs_f64()*1000.0);}
        }
    }
}
impl eframe::App for App {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        clear_background(self.preferences.dark)
    }
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested())
            && self.preferences.close_to_tray
            && self.desktop.is_some()
            && self.tray_ready
            && !self.quitting
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        let start=Instant::now();
        self.frame(ctx);
        self.diagnostic_sample(ctx,start);
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        if let Some(session)=&self.flicker {session.stop();}
        self.stop_http();
        self.plugins.take();
        if self.gate.dirty && !self.gate.invalid {
            if let Err(e) = store::save(&self.path, &self.config) {
                let message = format!("Échec de sauvegarde à la fermeture : {e}\n");
                eprintln!("{message}");
                let log = self.path.with_file_name("deckforge-error.log");
                let _ = std::fs::OpenOptions::new().create(true).append(true).open(log)
                    .and_then(|mut file| std::io::Write::write_all(&mut file, message.as_bytes()));
            }
        }
        self.engine.stop();
        let _ = self.device.send(Command::Disconnect);
    }
}
/// Business events keep running even when Windows stops drawing a hidden window.
pub struct Runner {
    app: std::sync::Arc<std::sync::Mutex<App>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl Runner {
    fn new(app: App, _ctx: egui::Context) -> Self {
        let app = std::sync::Arc::new(std::sync::Mutex::new(app));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let state = app.clone();
        let stopped = stop.clone();
        std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                {
                    let mut app = state.lock().unwrap_or_else(|e| e.into_inner());
                    app.background_tick();
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        Self { app, stop }
    }
}
impl App {
    fn plugin_face(&self,index:usize)->Option<&PluginFace>{
        let manager=self.plugins.as_ref()?;
        let id=manager.visible().values().find(|v|v.index==index)?.context.as_str();
        self.plugin_faces.get(id)
    }
    fn plugin_render_key(&self,index:usize,mut key:Key,face:&PluginFace)->Key {
        if !face.title.is_empty(){key.label=face.title.clone();}
        if face.image_bytes.is_none() {
            if let Some(v)=self.plugins.as_ref().and_then(|m|m.visible().values().find(|v|v.index==index)) {
                if let Some(action)=self.plugins.as_ref().into_iter().flat_map(|m|m.catalog.iter()).find(|p|p.uuid==v.plugin).and_then(|p|p.actions.iter().find(|a|a.uuid==v.action)) {
                    if key.label.is_empty(){if let Some(title)=action.titles.get(face.state as usize).and_then(Option::as_ref){key.label=title.clone();}}
                    if let Some(icon)=action.states.get(face.state as usize).and_then(Option::as_ref).or(action.icon.as_ref()) {
                        key.image=icon.to_string_lossy().into_owned();
                    }
                }
            }
        }
        key
    }
    fn send_plugin_face(&self,index:usize){
        let Some(face)=self.plugin_face(index) else {return};
        let key=self.plugin_render_key(index,self.applied.keys()[index].clone(),face);
        if let Ok(font)=crate::key_render::font(){
            if let Ok(jpeg)=crate::key_render::for_plugin(&key,self.applied.image_size,&font,face.image_bytes.as_deref(),face.flash.filter(|(_,when)|when.elapsed()<Duration::from_secs(1)).map(|(ok,_)|ok)){
                let _=self.device.send(Command::KeyImage(index,jpeg));
            }
        }
    }
    fn plugin_events(&mut self){
        let mut messages=Vec::new();
        if let Some(manager)=&self.plugins {while let Ok(message)=manager.rx.try_recv(){messages.push(message);if messages.len()>=100 {break;}}}
        for message in messages {match message {
            crate::plugin_runtime::Reply::Error(plugin,error)=>{self.plugin_errors.insert(plugin.clone(),error.clone());self.log(format!("Plugin {plugin} : {error}"));},
            crate::plugin_runtime::Reply::Message(plugin,message)=>self.plugin_message(&plugin,&message),
        }}
    }
    fn plugin_message(&mut self,plugin:&str,message:&serde_json::Value){
        use serde_json::json;
        let name=message["event"].as_str().unwrap_or("");
        let context=message["context"].as_str().unwrap_or("");
        let visible=self.plugins.as_ref().and_then(|m|m.visible().get(context)).filter(|v|v.plugin==plugin).cloned();
        let pi=message["__pi"].as_str();
        match name {
            "registerPlugin"=>{},
            "logMessage"=>{if let Some(text)=message["payload"]["message"].as_str(){self.log(format!("Plugin {plugin} : {}",text.chars().take(500).collect::<String>()));}},
            "setTitle"|"setState"|"setImage"|"showOk"|"showAlert"=>{
                if visible.is_none(){return;}
                let face=self.plugin_faces.entry(context.to_owned()).or_default();
                let previous=face.clone();
                match name {
                    "setTitle"=>{if let Some(title)=message["payload"]["title"].as_str(){face.title=title.chars().take(100).collect();}},
                    "setState"=>{if let Some(state)=message["payload"]["state"].as_u64(){face.state=state.min(255) as u8;}},
                    "showOk"=>face.flash=Some((true,Instant::now())),
                    "showAlert"=>face.flash=Some((false,Instant::now())),
                    "setImage"=>{if let Some(value)=message["payload"]["image"].as_str(){
                        if let Ok(image)=decode_plugin_image(value){face.image_bytes=image;}
                    }else if message["payload"]["image"].is_null(){face.image_bytes=None;}},
                    _=>{}
                }
                if face.title!=previous.title || face.image_bytes!=previous.image_bytes || face.state!=previous.state || face.flash.is_some() {
                    face.version=face.version.wrapping_add(1);
                    if let Some(v)=visible { self.send_plugin_face(v.index); }
                }
            },
            "setSettings"=>{
                if let (Some(v),Some(settings))=(visible,message["payload"]["settings"].as_object()){
                    if self.config.active==self.applied.active && self.config.page==self.applied.page && !self.gate.dirty {
                        if let Some(binding)=self.config.keys_mut()[v.index].plugin.as_mut(){binding.settings=json!(settings);}
                        if let Some(binding)=self.applied.keys_mut()[v.index].plugin.as_mut(){binding.settings=json!(settings);}
                        if let Err(e)=store::save(&self.path,&self.config){self.fail(format!("Réglages du plugin non sauvegardés : {e}"));}
                        else if let Some(manager)=&mut self.plugins {manager.sync(&self.applied);manager.send(plugin,json!({"event":"didReceiveSettings","action":v.action,"context":v.context,"device":"deckforge-d6","payload":{"settings":settings}}));}
                    }
                }
            },
            "getSettings"=>{if let (Some(v),Some(manager))=(visible,&self.plugins){manager.send(plugin,json!({"event":"didReceiveSettings","action":v.action,"context":v.context,"device":"deckforge-d6","payload":{"settings":v.settings},"__pi":pi}));}},
            "setGlobalSettings"=>{let settings=&message["payload"]["settings"];if settings.is_object(){let path=self.path.with_file_name("plugin-settings.json");let mut all=std::fs::read(&path).ok().and_then(|b|serde_json::from_slice::<serde_json::Value>(&b).ok()).filter(|v|v.is_object()).unwrap_or_else(||json!({}));all[plugin]=settings.clone();if let Err(e)=std::fs::write(path,serde_json::to_vec_pretty(&all).unwrap_or_default()){self.fail(format!("Réglages globaux non sauvegardés : {e}"));}else if let (Some(id),Some(manager))=(pi,&self.plugins){manager.send(plugin,json!({"event":"didReceiveGlobalSettings","payload":{"settings":settings},"__pi":id}));}}},
            "getGlobalSettings"=>{let all=std::fs::read(self.path.with_file_name("plugin-settings.json")).ok().and_then(|b|serde_json::from_slice::<serde_json::Value>(&b).ok()).unwrap_or_else(||json!({}));if let Some(manager)=&self.plugins {manager.send(plugin,json!({"event":"didReceiveGlobalSettings","payload":{"settings":all.get(plugin).cloned().unwrap_or_else(||json!({}))},"__pi":pi}));}},
            "openUrl"=>{if let Some(url)=message["payload"]["url"].as_str().filter(|u|u.starts_with("https://")||u.starts_with("http://")){let _=std::process::Command::new("rundll32.exe").arg("url.dll,FileProtocolHandler").arg(url).spawn();}},
            _=>{}
        }
    }
    fn background_tick(&mut self) {
        crate::obs_monitor::update(&self.config.obs, false);
        let obs_state = crate::obs_monitor::snapshot();
        if obs_state != self.obs_state {
            self.obs_state = obs_state;
            if self.wanted && !self.gate.dirty && !self.gate.invalid { self.upload(false); }
        }
        let expired=self.plugin_faces.iter_mut().filter_map(|(context,face)|{
            if face.flash.is_some_and(|(_,at)|at.elapsed()>=Duration::from_secs(1)){
                face.flash=None; face.version=face.version.wrapping_add(1);Some(context.clone())
            }else{None}
        }).collect::<Vec<_>>();
        for context in expired {if let Some(index)=self.plugins.as_ref().and_then(|m|m.visible().get(&context)).map(|v|v.index){self.send_plugin_face(index);}}
        if let Some(plugins)=&mut self.plugins { if !self.gate.dirty && self.pending.is_none() { plugins.sync(&self.applied); } }
        let initial=self.plugins.as_ref().map(|m|m.visible().values().filter(|v| !self.plugin_faces.contains_key(&v.context)).map(|v|(v.context.clone(),v.index,v.state)).collect::<Vec<_>>()).unwrap_or_default();
        for (context,index,state) in initial { self.plugin_faces.insert(context,PluginFace{state,..Default::default()}); self.send_plugin_face(index); }
        self.events();
        self.plugin_events();
        self.housekeeping();
        let due = self.long_press.due(Instant::now(), self.applied.long_press_ms);
        for index in due { self.execute_physical(index, true); }
        self.assistant_events();
        if self.gate.dirty
            && !self.gate.invalid
            && self.last_edit.elapsed() > Duration::from_millis(800)
            && self.config.validate_structure().is_ok()
        {
            self.save();
        }
    }
}
fn clear_background(dark: bool) -> [f32; 4] {
    let c = if dark { [24, 26, 31] } else { [237, 240, 245] };
    [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0]
}
impl eframe::App for Runner {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        clear_background(self.app.lock().unwrap_or_else(|e| e.into_inner()).preferences.dark)
    }
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        eframe::App::update(
            &mut *self.app.lock().unwrap_or_else(|e| e.into_inner()),
            ctx,
            frame,
        );
    }
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.stop.store(true, Ordering::SeqCst);
        eframe::App::on_exit(&mut *self.app.lock().unwrap_or_else(|e| e.into_inner()), gl);
    }
}
impl Drop for Runner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}
fn edit_condition(ui: &mut egui::Ui, condition: &mut crate::conditions::Condition, audio: &crate::audio::Snapshot, depth: usize) -> bool {
    use crate::conditions::Condition as C;
    let mut changed = false;
    let variants = [
        ("foreground_app", "Application au premier plan"), ("app_running", "Application ouverte (arrière-plan inclus)"),
        ("window_open", "Fenêtre ouverte"), ("app_audio_playing", "Session audio active"),
        ("app_muted", "Application muette"), ("app_volume_compare", "Volume d’application"),
        ("device_muted", "Périphérique muet"), ("device_volume_compare", "Volume du périphérique"),
        ("audio_device_present", "Périphérique audio actif"), ("default_output_is", "Sortie par défaut"),
        ("variable_compare", "Variable"), ("not", "NON"), ("all", "Toutes (ET)"), ("any", "Au moins une (OU)")
    ];
    let mut selected = condition.kind().to_owned();
    egui::ComboBox::from_id_salt(("condition_kind", depth)).selected_text(condition.label()).show_ui(ui, |ui| {
        for (id, label) in variants { if depth < 8 || !["not", "all", "any"].contains(&id) {
            changed |= ui.selectable_value(&mut selected, id.into(), label).changed();
        } }
    });
    if selected != condition.kind() {
        *condition = match selected.as_str() {
            "foreground_app" => C::ForegroundApp { exe: "explorer.exe".into() },
            "app_running" => C::AppRunning { exe: "explorer.exe".into() },
            "window_open" => C::WindowOpen { title: "Chrome".into() },
            "app_audio_playing" => C::AppAudioPlaying { exe: "foreground".into() },
            "app_muted" => C::AppMuted { exe: "foreground".into() },
            "app_volume_compare" => C::AppVolumeCompare { exe: "foreground".into(), op: "above".into(), value: 50 },
            "device_muted" => C::DeviceMuted { input: false, device: "default".into() },
            "device_volume_compare" => C::DeviceVolumeCompare { input: false, device: "default".into(), op: "above".into(), value: 50 },
            "audio_device_present" => C::AudioDevicePresent { name_contains: "Casque".into(), input: false },
            "default_output_is" => C::DefaultOutputIs { name_contains: "Casque".into() },
            "variable_compare" => C::VariableCompare { name: "mode".into(), op: "equals".into(), value: String::new() },
            "not" => C::Not { condition: Box::new(C::default()) },
            "all" => C::All { conditions: vec![C::default()] },
            "any" => C::Any { conditions: vec![C::default()] },
            _ => unreachable!(),
        };
    }
    match condition {
        C::ForegroundApp { exe } | C::AppRunning { exe } => {
            ui.label("Nom d’exécutable (.exe) :");
            changed |= condition_exe(ui, exe);
        }
        C::WindowOpen { title } => { ui.label("Le titre contient :"); changed |= ui.text_edit_singleline(title).changed(); }
        C::AppAudioPlaying { exe } | C::AppMuted { exe } => { changed |= condition_audio_exe(ui, exe); }
        C::AppVolumeCompare { exe, op, value } => {
            changed |= condition_audio_exe(ui, exe);
            changed |= condition_operator(ui, op);
            changed |= ui.add(egui::Slider::new(value, 0..=100).text("%" )).changed();
        }
        C::DeviceMuted { input, device } | C::DeviceVolumeCompare { input, device, .. } => {
            changed |= ui.checkbox(input, "Microphone (sinon sortie)").changed();
            egui::ComboBox::from_id_salt("condition_device").selected_text(if device == "default" { "Par défaut" } else { device.as_str() }).show_ui(ui, |ui| {
                changed |= ui.selectable_value(device, "default".into(), "Par défaut").changed();
                for endpoint in audio.devices.iter().filter(|d| d.input == *input) {
                    changed |= ui.selectable_value(device, endpoint.id.clone(), &endpoint.name).changed();
                }
            });
            egui::CollapsingHeader::new("Options avancées").id_salt("condition_device_id").show(ui, |ui| {
                changed |= ui.text_edit_singleline(device).changed();
            });
        }
        C::AudioDevicePresent { name_contains, input } => {
            changed |= ui.checkbox(input, "Microphone (sinon sortie)").changed();
            let options: Vec<_> = audio.devices.iter().filter(|d| d.input == *input).collect();
            egui::ComboBox::from_id_salt("condition_device_name").selected_text(name_contains.as_str()).show_ui(ui, |ui| {
                for endpoint in options { changed |= ui.selectable_value(name_contains, endpoint.name.clone(), &endpoint.name).changed(); }
            });
            ui.label("Le nom contient :"); changed |= ui.text_edit_singleline(name_contains).changed();
        }
        C::DefaultOutputIs { name_contains } => {
            egui::ComboBox::from_id_salt("condition_default_output").selected_text(name_contains.as_str()).show_ui(ui, |ui| {
                for endpoint in audio.devices.iter().filter(|d| !d.input) { changed |= ui.selectable_value(name_contains, endpoint.name.clone(), &endpoint.name).changed(); }
            });
            ui.label("Le nom contient :"); changed |= ui.text_edit_singleline(name_contains).changed();
        }
        C::VariableCompare { name, op, value } => {
            ui.label("Nom de variable"); changed |= ui.text_edit_singleline(name).changed();
            egui::ComboBox::from_id_salt("variable_operator").selected_text(op.as_str()).show_ui(ui, |ui| {
                for (id, label) in [("equals", "Égale"), ("not_equals", "Différente"), ("contains", "Contient"), ("greater", "Supérieure"), ("less", "Inférieure")] {
                    changed |= ui.selectable_value(op, id.into(), label).changed();
                }
            });
            ui.label("Valeur"); changed |= ui.text_edit_singleline(value).changed();
        }
        C::Not { condition } => { ui.indent("not_condition", |ui| { changed |= edit_condition(ui, condition, audio, depth + 1); }); }
        C::All { conditions } | C::Any { conditions } => {
            let mut remove = None;
            for (i, child) in conditions.iter_mut().enumerate() {
                ui.push_id(i, |ui| { ui.horizontal(|ui| { ui.label(format!("Condition {}", i + 1));
                    if ui.small_button("Retirer").clicked() { remove = Some(i); }
                });
                ui.indent(i, |ui| { changed |= edit_condition(ui, child, audio, depth + 1); }); });
            }
            if let Some(i) = remove { if conditions.len() > 1 { conditions.remove(i); changed = true; } }
            if depth < 8 && conditions.len() < 32 && ui.button("+ Ajouter une condition").clicked() { conditions.push(C::default()); changed = true; }
        }
    }
    if let C::DeviceVolumeCompare { op, value, .. } = condition {
        changed |= condition_operator(ui, op);
        changed |= ui.add(egui::Slider::new(value, 0..=100).text("%" )).changed();
    }
    changed
}
fn condition_exe(ui: &mut egui::Ui, exe: &mut String) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt("exe_suggestions").selected_text(exe.as_str()).show_ui(ui, |ui| {
        for name in crate::conditions::running_exes() { changed |= ui.selectable_value(exe, name.clone(), name).changed(); }
    });
    changed | ui.text_edit_singleline(exe).changed()
}
fn condition_audio_exe(ui: &mut egui::Ui, exe: &mut String) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt("condition_audio_exe").selected_text(if exe == "foreground" { "Application au premier plan" } else { exe.as_str() }).show_ui(ui, |ui| {
        changed |= ui.selectable_value(exe, "foreground".into(), "Application au premier plan").changed();
        for name in crate::conditions::running_exes() { changed |= ui.selectable_value(exe, name.clone(), name).changed(); }
    });
    ui.label("Ou saisir un exécutable (.exe) :");
    changed | ui.text_edit_singleline(exe).changed()
}
fn condition_operator(ui: &mut egui::Ui, op: &mut String) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt("volume_operator").selected_text(if op == "above" { "Au-dessus de" } else { "En dessous de" }).show_ui(ui, |ui| {
        changed |= ui.selectable_value(op, "above".into(), "Au-dessus de").changed();
        changed |= ui.selectable_value(op, "below".into(), "En dessous de").changed();
    });
    changed
}
#[allow(clippy::too_many_arguments)] // UI editor must carry the stable dialog destination through nested steps.
fn edit_step(
    ui: &mut egui::Ui,
    step: &mut Step,
    profiles: &[String],
    pages: &[crate::model::Page],
    record: &mut bool,
    audio: &crate::audio::Snapshot,
    pick_audio_icon: &mut Option<usize>,
    tx: &Sender<Option<DialogEffect>>,
    target: &StepTarget,
) -> bool {
    let mut changed = false;
    match step {
        Step::Disabled { .. } => unreachable!("atelier passe une étape active"),
        Step::MouseClick { button, double } => {
            egui::ComboBox::from_id_salt("mouse_button").selected_text(match button.as_str() { "left" => "Gauche", "right" => "Droit", _ => "Milieu" }).show_ui(ui, |ui| {
                for (id, label) in [("left", "Gauche"), ("right", "Droit"), ("middle", "Milieu")] { changed |= ui.selectable_value(button, id.into(), label).changed(); }
            });
            changed |= ui.checkbox(double, "Double clic").changed();
        }
        Step::MouseMove { x, y, relative } => {
            ui.horizontal(|ui| { ui.label("X"); changed |= ui.add(egui::DragValue::new(x).range(-32768..=32767)).changed(); ui.label("Y"); changed |= ui.add(egui::DragValue::new(y).range(-32768..=32767)).changed(); });
            changed |= ui.checkbox(relative, "Déplacement relatif").changed();
        }
        Step::MouseScroll { amount } => { ui.label("Défilement (crans)"); changed |= ui.add(egui::DragValue::new(amount).range(-100..=100)).changed(); }
        Step::PlaySound { path, volume } => { ui.label("Fichier WAV ou MP3"); changed |= ui.text_edit_singleline(path).changed(); ui.label("Volume"); changed |= ui.add(egui::Slider::new(volume, 0..=100)).changed(); }
        Step::StopSounds => { ui.label("Arrête tous les sons lancés par DeckForge."); }
        Step::Toggle { .. } => { ui.label("Alterne les états 1 et 2 à chaque appui. Modifie leurs blocs dans l’Atelier."); }
        Step::UdpSend { host, port, text } => { ui.label("Hôte UDP"); changed |= ui.text_edit_singleline(host).changed(); ui.label("Port"); changed |= ui.add(egui::DragValue::new(port).range(1..=65535)).changed(); ui.label("Message"); changed |= ui.text_edit_multiline(text).changed(); }
        Step::HttpRequest { method, url, body, content_type } => { ui.label("Méthode"); egui::ComboBox::from_id_salt("http_method").selected_text(method.as_str()).show_ui(ui, |ui| { for choice in ["GET", "POST", "PUT"] { changed |= ui.selectable_value(method, choice.into(), choice).changed(); } }); ui.label("Adresse HTTP (sans HTTPS)"); changed |= ui.text_edit_singleline(url).changed(); ui.label("Corps"); changed |= ui.text_edit_multiline(body).changed(); ui.label("Type de contenu"); changed |= ui.text_edit_singleline(content_type).changed(); }
        Step::D6Brightness { mode, value } => {
            ui.label("Luminosité du pavé D6");
            egui::ComboBox::from_id_salt("d6_brightness_mode").selected_text(match mode.as_str() { "up" => "Augmenter", "down" => "Diminuer", _ => "Régler à" }).show_ui(ui, |ui| {
                for (id, label) in [("set", "Régler à"), ("up", "Augmenter"), ("down", "Diminuer")] { changed |= ui.selectable_value(mode, id.into(), label).changed(); }
            });
            changed |= ui.add(egui::Slider::new(value, 0..=100).text("%" )).changed();
        }
        Step::DeviceVolume { input, device, action, step, level } => {
            ui.label("Volume du périphérique (pas celui d'une application)");
            changed |= ui.checkbox(input, "Microphone (sinon sortie)").changed();
            let options: Vec<_> = audio.devices.iter().filter(|d| d.input == *input).collect();
            let selected = if device == "default" { "Par défaut".to_string() }
                else { options.iter().find(|d| d.id == *device).map(|d| d.name.clone()).unwrap_or_else(|| device.clone()) };
            egui::ComboBox::from_id_salt("device_volume_target").selected_text(selected).show_ui(ui, |ui| {
                changed |= ui.selectable_value(device, "default".into(), "Par défaut").changed();
                for endpoint in &options {
                    changed |= ui.selectable_value(device, endpoint.id.clone(), &endpoint.name).changed();
                }
            });
            ui.label("Action");
            egui::ComboBox::from_id_salt("device_volume_action").selected_text(match action.as_str() {
                "up" => "Augmenter", "down" => "Diminuer", "set" => "Régler à", "mute" => "Couper", "unmute" => "Rétablir", _ => "Basculer muet",
            }).show_ui(ui, |ui| {
                for (id, label) in [("up", "Augmenter"), ("down", "Diminuer"), ("set", "Régler à"), ("mute", "Couper"), ("unmute", "Rétablir"), ("toggle_mute", "Basculer muet")] {
                    changed |= ui.selectable_value(action, id.into(), label).changed();
                }
            });
            if action == "up" || action == "down" { changed |= ui.add(egui::Slider::new(step, 1..=100).text("Pas (points)")).changed(); }
            if action == "set" { changed |= ui.add(egui::Slider::new(level, 0..=100).text("Niveau (%)")).changed(); }
            egui::CollapsingHeader::new("Options avancées").id_salt("device_volume_advanced").show(ui, |ui| {
                ui.label("ID du périphérique (ou default) :");
                changed |= ui.text_edit_singleline(device).changed();
            });
        }
        Step::AppVolume { target, action, step, level, include_children } => {
            ui.label("Cible");
            let mut foreground = target == "foreground";
            egui::ComboBox::from_id_salt("app_volume_target").selected_text(if foreground { "Application au premier plan" } else { "Application précise" }).show_ui(ui, |ui| {
                changed |= ui.selectable_value(&mut foreground, true, "Application au premier plan").changed();
                changed |= ui.selectable_value(&mut foreground, false, "Application précise").changed();
            });
            if foreground { if target != "foreground" { *target = "foreground".into(); changed = true; } }
            else {
                if target == "foreground" { *target = "chrome.exe".into(); changed = true; }
                ui.label("Nom du programme (ex. chrome.exe)");
                changed |= ui.text_edit_singleline(target).changed();
            }
            ui.label("Action");
            egui::ComboBox::from_id_salt("app_volume_action").selected_text(match action.as_str() {
                "up" => "Augmenter", "down" => "Diminuer", "set" => "Régler à", "mute" => "Couper", "unmute" => "Rétablir", _ => "Basculer muet",
            }).show_ui(ui, |ui| {
                for (id, label) in [("up", "Augmenter"), ("down", "Diminuer"), ("set", "Régler à"), ("mute", "Couper"), ("unmute", "Rétablir"), ("toggle_mute", "Basculer muet")] {
                    changed |= ui.selectable_value(action, id.into(), label).changed();
                }
            });
            if action == "up" || action == "down" { changed |= ui.add(egui::Slider::new(step, 1..=100).text("Pas (points)")).changed(); }
            if action == "set" { changed |= ui.add(egui::Slider::new(level, 0..=100).text("Niveau (%)")).changed(); }
            egui::CollapsingHeader::new("Options avancées").id_salt("app_volume_advanced").show(ui, |ui| {
                changed |= ui.checkbox(include_children, "Inclure les processus enfants").changed();
            });
        }
        Step::D6Sleep => { ui.label("Met le D6 en veille maintenant. Un appui le réveille sans lancer la macro."); }
        Step::CloseWindow => { ui.label("Ferme la fenêtre active (jamais DeckForge)."); }
        Step::ObsScene { name } => { ui.label("Scène OBS"); changed |= obs_entry(ui, name, &crate::obs_monitor::snapshot().scenes); }
        Step::ObsMute { input, mode } => { ui.label("Entrée audio OBS"); changed |= obs_entry(ui, input, &crate::obs_monitor::snapshot().inputs); obs_choice(ui, mode, &["toggle", "mute", "unmute"], &mut changed); }
        Step::ObsSourceVisibility { scene, source, mode } => { ui.label("Scène puis source OBS"); changed |= obs_entry(ui, scene, &crate::obs_monitor::snapshot().scenes); changed |= obs_entry(ui, source, crate::obs_monitor::snapshot().scene_items.get(scene).map(Vec::as_slice).unwrap_or(&[])); obs_choice(ui, mode, &["toggle", "show", "hide"], &mut changed); }
        Step::ObsRecord { mode } => obs_choice(ui, mode, &["toggle", "start", "stop", "pause"], &mut changed),
        Step::ObsStream { mode } | Step::ObsVirtualCam { mode } => obs_choice(ui, mode, &["toggle", "start", "stop"], &mut changed),
        Step::ObsReplay { mode } => obs_choice(ui, mode, &["toggle", "start", "stop", "save"], &mut changed),
        Step::Delay { ms } => {
            ui.horizontal(|ui| {
                changed |= ui
                    .add(egui::DragValue::new(ms).range(0..=3_600_000).speed(50))
                    .changed();
                ui.label("ms");
            });
        }
        Step::Hotkey { keys } => {
            ui.horizontal(|ui| {
                changed |= ui.text_edit_singleline(keys).changed();
                if ui.button("Enregistrer…").clicked() {
                    *record = true;
                }
            });
            ui.label(RichText::new("Ex. CTRL+SHIFT+S · ALT+TAB · F13").small());
        }
        Step::Text { text } => {
            changed |= ui
                .add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(2)
                        .desired_width(ui.available_width()),
                )
                .changed();
        }
        Step::Launch { program, args } => {
            ui.horizontal(|ui| {
                changed |= ui.text_edit_singleline(program).changed();
                if ui.small_button("…").clicked() {
                    choose_step_file(tx.clone(),target.clone(),false);
                }
            });
            ui.label(RichText::new("Arguments : un par ligne, sans guillemets ajoutés").small());
            let mut text = args.join("\n");
            if ui
                .add(
                    egui::TextEdit::multiline(&mut text)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                )
                .changed()
            {
                *args = if text.is_empty() {
                    vec![]
                } else {
                    text.split('\n').map(str::to_string).collect()
                };
                changed = true;
            }
        }
        Step::Url { url } => {
            changed |= ui.text_edit_singleline(url).changed();
        }
        Step::WaitWindow {
            title,
            timeout_ms,
            focus,
        } => {
            ui.label("Le titre contient (sans distinction majuscule/minuscule)");
            changed |= ui.text_edit_singleline(title).changed();
            ui.horizontal(|ui| {
                ui.label("Délai max.");
                changed |= ui
                    .add(
                        egui::DragValue::new(timeout_ms)
                            .range(100..=3_600_000)
                            .speed(100),
                    )
                    .changed();
                ui.label("ms");
            });
            changed |= ui
                .checkbox(focus, "Mettre cette fenêtre au premier plan")
                .changed();
        }
        Step::Media { command } => {
            let items = [
                ("play_pause", "Lecture / pause"),
                ("next", "Piste suivante"),
                ("previous", "Piste précédente"),
                ("stop", "Arrêter"),
                ("mute", "Muet"),
                ("volume_up", "Volume +"),
                ("volume_down", "Volume −"),
            ];
            let label = items
                .iter()
                .find(|(id, _)| *id == command)
                .map(|(_, label)| *label)
                .unwrap_or("Choisir");
            egui::ComboBox::from_id_salt("media")
                .selected_text(label)
                .show_ui(ui, |ui| {
                    for (id, label) in items {
                        changed |= ui.selectable_value(command, id.into(), label).changed();
                    }
                });
        }
        Step::Folder { .. } | Step::Page { .. } => {
            let folder = matches!(step, Step::Folder { .. });
            let id = match step {
                Step::Folder { id } | Step::Page { id } => id,
                _ => unreachable!(),
            };
            egui::ComboBox::from_id_salt("navigation_target")
                .selected_text(
                    pages
                        .iter()
                        .find(|p| p.id == *id)
                        .map(|p| p.name.as_str())
                        .unwrap_or("Page 1"),
                )
                .show_ui(ui, |ui| {
                    if !folder {
                        changed |= ui.selectable_value(id, String::new(), "Page 1").changed();
                    }
                    for page in pages.iter().filter(|p| p.folder == folder) {
                        changed |= ui
                            .selectable_value(id, page.id.clone(), &page.name)
                            .changed();
                    }
                });
        }
        step @ Step::AudioSwitch { .. } => {
            changed |= icon_ui::audio_options(ui, step, audio, pick_audio_icon, tx, target);
        }
        Step::Notify { title, message } => {
            ui.label("Titre"); changed |= ui.text_edit_singleline(title).changed();
            ui.label("Message (variables : {nom} ; {{ pour une accolade)"); changed |= ui.text_edit_multiline(message).changed();
        }
        Step::WaitUntil { condition, timeout_ms, poll_ms } => {
            changed |= edit_condition(ui, condition, audio, 0);
            egui::CollapsingHeader::new("Options avancées").id_salt("wait_until_advanced").show(ui, |ui| {
                ui.label("Délai maximal (ms)");
                changed |= ui.add(egui::DragValue::new(timeout_ms).range(50..=600_000)).changed();
                ui.label("Intervalle de vérification (ms)");
                changed |= ui.add(egui::DragValue::new(poll_ms).range(50..=600_000)).changed();
            });
        }
        Step::IfCondition { condition, .. } => {
            changed |= edit_condition(ui, condition, audio, 0);
            ui.label("Configure ALORS et SINON dans l’Atelier blocs.");
        }
        Step::IfWindow { title, .. } => {
            ui.label("Le titre de la fenêtre contient");
            changed |= ui.text_edit_singleline(title).changed();
            ui.label("Configure ALORS et SINON dans l’Atelier blocs.");
        }
        Step::Repeat { count, .. } => {
            ui.horizontal(|ui| {
                ui.label("Répéter");
                changed |= ui.add(egui::DragValue::new(count).range(1..=100)).changed();
                ui.label("fois les blocs de l’Atelier");
            });
        }
        Step::SetVariable { name, value } | Step::IfVariable { name, value, .. } => {
            ui.horizontal(|ui| {
                ui.label("Variable");
                changed |= ui.text_edit_singleline(name).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Valeur");
                changed |= ui.text_edit_singleline(value).changed();
            });
        }
        Step::AutoHotkey {
            executable,
            script,
            args,
        } => {
            ui.horizontal(|ui| {
                ui.label("AutoHotkey v2");
                changed |= ui.text_edit_singleline(executable).changed();
                if ui.button("Parcourir…").clicked() {
                    choose_step_file(tx.clone(),target.clone(),true);
                }
            });
            changed |= ui
                .add(
                    egui::TextEdit::multiline(script)
                        .code_editor()
                        .desired_rows(8)
                        .desired_width(ui.available_width()),
                )
                .changed();
            let mut text = args.join("\n");
            ui.label("Arguments : un par ligne");
            if ui.text_edit_multiline(&mut text).changed() {
                *args = text.lines().map(String::from).collect();
                changed = true;
            }
            ui.label(RichText::new("Le script agit sur ton PC lors de l’exécution. Arrêter la macro termine le processus AutoHotkey lancé par DeckForge.").small());
        }
        Step::Back | Step::PageNext | Step::PagePrevious => {
            ui.label("Navigation dans ce profil, aussi disponible depuis les touches du D6.");
        }
        Step::Profile { name } => {
            egui::ComboBox::from_id_salt("profile_action")
                .selected_text(&*name)
                .show_ui(ui, |ui| {
                    for p in profiles {
                        changed |= ui.selectable_value(name, p.clone(), p).changed();
                    }
                });
        }
    }
    changed
}

fn step_summary(step: &Step) -> String {
    match step {
        Step::Hotkey { keys } => keys.clone(),
        Step::Delay { ms } => format!("Attendre {ms} ms"),
        Step::MouseClick { button, double } => format!("Clic {button}{}", if *double { " double" } else { "" }),
        Step::MouseMove { x, y, relative } => format!("Souris {x}, {y} ({})", if *relative { "relatif" } else { "absolu" }),
        Step::MouseScroll { amount } => format!("Défiler de {amount} crans"),
        Step::PlaySound { path, volume } => format!("Son : {path} ({volume} %)"),
        Step::IfCondition { condition, .. } => format!("Si {}", condition.summary()),
        Step::WaitUntil { condition, timeout_ms, .. } => format!("Attendre que {} ({} ms max)", condition.summary(), timeout_ms),
        Step::Notify { title, message } => format!("Notification « {title} » : {}", message.chars().take(50).collect::<String>()),
        Step::AppVolume { .. } => step.app_volume_summary().unwrap_or_default(),
        Step::DeviceVolume { .. } => step.device_volume_summary().unwrap_or_default(),
        Step::StopSounds => "Arrêter les sons".into(),
        Step::Toggle { a, b } => format!("Basculer 1 / 2 ({} / {} blocs)", a.len(), b.len()),
        Step::UdpSend { host, port, .. } => format!("UDP : {host}:{port}"),
        Step::HttpRequest { method, url, .. } => format!("HTTP {method} : {url}"),
        Step::D6Brightness { mode, value } => format!("Luminosité D6 : {mode} {value} %"),
        Step::D6Sleep => "Veille du D6".into(),
        Step::CloseWindow => "Fermer la fenêtre active".into(),
        Step::Media { command } => format!("Média : {command}"),
        Step::Url { url } => url.clone(),
        Step::Text { .. } => "Texte".into(),
        Step::Launch { program, .. } => format!("Lancer {program}"),
        Step::WaitWindow { title, .. } => format!("Attendre {title}"),
        Step::Profile { name } => format!("Profil {name}"),
        Step::Folder { id } => format!("Dossier {id}"),
        Step::Page { id } => format!("Page {id}"),
        Step::Back => "Retour".into(),
        Step::PageNext => "Page suivante".into(),
        Step::PagePrevious => "Page précédente".into(),
        _ => step.name().into(),
    }
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;

#[path = "ui_extras.rs"]
mod extras;

#[path = "ui_icons.rs"]
mod icon_ui;

#[cfg(test)]
mod dialog_tests {
    use super::*;
    #[test]
    fn dialog_worker_result_applies_on_the_next_ui_tick_without_native_window() {
        let dir=tempfile::tempdir().unwrap();
        let mut app=App::init(&egui::Context::default(),Some(dir.path().join("config.json")));
        app.start_dialog(|| Some(Box::new(|app: &mut App| app.config.brightness=17)));
        assert!(app.dialog_pending);
        let result=app.dialog_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        app.dialog_tx.send(result).unwrap();
        app.dialog_results();
        assert!(!app.dialog_pending);
        assert_eq!(app.config.brightness,17);
    }
}

#[cfg(test)]
mod automatic_profile_sleep_tests {
    use super::*;
    #[test]
    fn legacy_config_defaults_and_new_fields_round_trip() {
        let old = serde_json::to_value(Config::default()).unwrap();
        let mut old = old;
        for field in ["auto_profile", "sleep_after_min", "sleep_brightness"] { old.as_object_mut().unwrap().remove(field); }
        old["profiles"][0].as_object_mut().unwrap().remove("apps");
        let mut loaded: Config = serde_json::from_value(old).unwrap();
        assert!(!loaded.auto_profile);
        assert_eq!(loaded.sleep_after_min, 0);
        assert_eq!(loaded.sleep_brightness, 0);
        assert!(loaded.profiles[0].apps.is_empty());
        loaded.profiles[0].apps.push("OBS64.EXE".into());
        loaded.auto_profile = true;
        loaded.sleep_after_min = 15;
        assert_eq!(serde_json::from_str::<Config>(&serde_json::to_string(&loaded).unwrap()).unwrap().profiles[0].apps, vec!["OBS64.EXE"]);
        loaded.sleep_after_min = 241;
        assert!(loaded.validate().is_err());
    }
    #[test]
    fn matching_and_base_fallback() {
        let mut c = Config::default();
        c.profiles.push(Profile::new("OBS".into()));
        c.profiles[1].apps.push("obs64.exe".into());
        c.auto_profile = true;
        assert_eq!(choose_profile(&c, Some("OBS64.EXE"), 0), 1);
        assert_eq!(choose_profile(&c, Some("other.exe"), 0), 0);
        assert_eq!(choose_profile(&c, None, 0), 0);
        c.auto_profile = false;
        assert_eq!(choose_profile(&c, Some("obs64.exe"), 0), 0);
    }
    #[test]
    fn macro_blocks_auto_switch_until_finished() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::init(&egui::Context::default(), Some(dir.path().join("config.json")));
        app.config.profiles.push(Profile::new("OBS".into()));
        app.config.profiles[1].apps.push("obs64.exe".into());
        app.config.auto_profile = true;
        app.foreground = Some("obs64.exe".into());
        app.engine.running.store(true, Ordering::SeqCst);
        app.update_auto_profile();
        assert_eq!(app.config.active, 0);
        app.engine.running.store(false, Ordering::SeqCst);
        app.update_auto_profile();
        assert_eq!(app.config.active, 1);
        app.foreground = Some("other.exe".into());
        app.update_auto_profile();
        assert_eq!(app.config.active, 0);
    }
    #[test]
    fn sleep_timer_and_first_press_is_swallowed() {
        let now = Instant::now();
        let mut sleep = SleepState::new(now);
        assert!(!sleep.due(now + Duration::from_secs(59), 1));
        assert!(sleep.due(now + Duration::from_secs(60), 1));
        assert!(!sleep.due(now + Duration::from_secs(61), 1));
        assert!(sleep.press(now + Duration::from_secs(62)));
        assert!(!sleep.press(now + Duration::from_secs(63)));
        assert!(!sleep.due(now + Duration::from_secs(999), 0));
    }
}

fn obs_choice(ui: &mut egui::Ui, mode: &mut String, options: &[&str], changed: &mut bool) {
    egui::ComboBox::from_id_salt(ui.next_auto_id()).selected_text(mode.as_str()).show_ui(ui, |ui| {
        for option in options { *changed |= ui.selectable_value(mode, (*option).to_owned(), *option).changed(); }
    });
}

fn obs_entry(ui: &mut egui::Ui, value: &mut String, options: &[String]) -> bool {
    let mut changed = ui.text_edit_singleline(value).changed();
    if !options.is_empty() {
        egui::ComboBox::from_id_salt(ui.next_auto_id()).selected_text("Choisir…").show_ui(ui, |ui| {
            for item in options {
                if ui.selectable_label(value == item, item).clicked() { *value = item.clone(); changed = true; }
            }
        });
    }
    changed
}
