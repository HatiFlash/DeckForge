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
use crate::iconset::{self, Family, Ico};
use crate::motion;
use crate::i18n::{tr, trf};
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
#[path = "ui_plugin_market.rs"]
mod plugin_market_ui;
#[path = "ui_plugin_validation.rs"]
mod plugin_validation_harness;
pub(crate) use plugin_validation_harness::validate_oracle;
#[cfg(test)]
pub(crate) use plugin_validation_harness::{validate_restart_persistence, validate_witness};

#[path = "ui_workshop_chat.rs"]
mod workshop_chat_ui;

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

#[derive(Clone)]
struct DeviceGrid { name: String, keys: usize, rows: usize, cols: usize, experimental: bool }
/// A video import that is waiting for FFmpeg to be installed.
#[derive(Clone)]
struct FfmpegOffer { video: PathBuf, assets: PathBuf, size: u32, profile: usize, page: String, index: usize }
impl Default for DeviceGrid {
    fn default() -> Self { Self { name: "Aucun périphérique".into(), keys: 15, rows: 3, cols: 5, experimental: false } }
}
impl DeviceGrid {
    fn new(name: String, keys: u8, rows: u8, cols: u8, experimental: bool) -> Option<Self> {
        let (keys, rows, cols) = (keys as usize, rows as usize, cols as usize);
        (keys > 0 && keys <= 32 && rows > 0 && cols > 0 && rows * cols == keys)
            .then_some(Self { name, keys, rows, cols, experimental })
    }
    fn total_width(&self, width: f32) -> f32 { width * self.cols as f32 + 14.0 * (self.cols - 1) as f32 + 48.0 }
    fn total_height(&self, width: f32) -> f32 { width * self.rows as f32 + 14.0 * (self.rows - 1) as f32 + 68.0 }
    fn key_width(&self, available_width: f32, available_height: f32) -> f32 {
        ((available_width - 48.0 - 14.0 * (self.cols - 1) as f32) / self.cols as f32)
            .min((available_height - 68.0) / self.rows as f32 - 14.0 * (self.rows - 1) as f32 / self.rows as f32)
            .floor().clamp(40.0, 320.0)
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
    state_monitor: Sender<crate::state_display::Command>,
    state_generation: u64,
    state_values: [Option<bool>; 32],
    device_model: Option<DeviceGrid>,
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
    show_device_report: bool,
    hardware_report: Option<crate::hardware_report::Report>,
    show_hardware_test: bool,
    hardware_test: Option<crate::devices::test_assistant::Session>,
    show_saved_variables: bool,
    show_about: bool,
    crop_session: Option<crate::ui_media_crop::CropSession>,
    ffmpeg_offer: Option<FfmpegOffer>,
    ffmpeg_installing: bool,
    update_result: Option<Result<crate::app_update::Status, String>>,
    update_rx: Option<Receiver<about::UpdateEvent>>,
    update_phase: Option<&'static str>,
    update_confirm: Option<crate::app_update::Latest>,
    update_installed: bool,
    startup_prompts: bool,
    startup_checked: bool,
    startup_rx: Option<Receiver<Result<crate::app_update::Status, String>>>,
    startup_offer: Option<crate::app_update::Latest>,
    whats_new: bool,
    whats_new_notes: String,
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
    /// Demandes externes capturées dans une App de validation ; jamais exécutées.
    plugin_external_effects: Option<Vec<String>>,
    plugin_report_cache: HashMap<String, Option<crate::plugin_validation::Report>>,
    plugin_oracle_cache: HashMap<String, Option<crate::plugin_validation::OracleFile>>,
    plugin_real_confirmation: Option<crate::plugins::Plugin>,
    plugin_inventory: Option<std::sync::Arc<plugin_store_ui::Inventory>>,
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
    import_reports: Vec<crate::elgato::Report>,
    dialog_tx: Sender<Option<DialogEffect>>,
    dialog_rx: Receiver<Option<DialogEffect>>,
    dialog_pending: bool,
    page_manager: bool,
    delete_page: Option<String>,
    desktop: Option<crate::desktop::Desktop>,
    audio: crate::audio::Snapshot,
    obs_state: crate::obs_monitor::Snapshot,
    running_block: Vec<usize>,
    workshop_journal: crate::workshop_journal::Journal,
    open_requested: bool,
    /// Images restantes pendant lesquelles la fenêtre est recachée (eframe l'affiche après la première image).
    hide_frames: u8,
    tray_ready: bool,
    quitting: bool,
    show_workflow: bool,
    workshop_chat: workshop_chat_ui::WorkshopChat,
    local_ai: crate::local_llm::Runtime,
    local_ai_check: Option<crate::local_llm::Job>,
    local_ai_install: Option<crate::local_llm::Install>,
    local_ai_status: String,
    /// Onglet de la fenêtre Assistant IA : 0 = IA locale, 1 = IA connectée (MCP).
    assistant_tab: u8,
    /// Onglet des Réglages : 0 = Général, 1 = Périphérique, 2 = Diagnostic.
    settings_tab: u8,
    show_assets: bool,
    show_desktop_settings: bool,
    available_icons: Vec<crate::assets::Icon>,
    available_plugins: Vec<crate::assets::Plugin>,
    icon_search: String,
    icon_pack: String,
    asset_tab: usize,
    trigger_recording: bool,
    trigger_kind: String,
    trigger_value: String,
    trigger_days: Vec<u8>,
    trigger_minutes: u32,
    trigger_input: bool,
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
        Self::init_with(ctx, override_path, true)
    }
    /// `configure_variables=false` keeps the process-wide saved variables untouched; used by the
    /// background plugin validation, which runs while the real application is open.
    fn init_with(ctx: &egui::Context, override_path: Option<PathBuf>, configure_variables: bool) -> Self {
        let isolated = override_path.is_some();
        theme::install(ctx);
        let path = override_path.unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap_or_else(|_| PathBuf::from("NovyDeck.exe"))
                .with_file_name("config.json")
        });
        let mut load_error = None;
        let config = if path.exists() {
            store::load(&path).unwrap_or_else(|e| {
                load_error = Some(trf(
                    "Configuration illisible : {error}. Importe une sauvegarde pour la récupérer.",
                    &[("error", &e)]
                ));
                Config::default()
            })
        } else {
            Config::default()
        };
        if !configure_variables {
        } else if let Err(e) = crate::persistent_variables::configure(&path.with_file_name("variables.json")) {
            load_error = Some(trf("Variables enregistrées illisibles : {error}", &[("error", &e)]));
        }
        let smoke = !isolated && std::env::args().any(|a| a == "--ui-smoke");
        let preferences_path = path.with_file_name("preferences.json");
        let preferences_existed = preferences_path.exists();
        let preferences_result = Preferences::load(&preferences_path);
        let preferences_valid = preferences_result.is_ok();
        let mut preferences = preferences_result.unwrap_or_default();
        let whats_new_notes = include_str!(concat!(env!("OUT_DIR"), "/release_notes.md")).to_owned();
        let startup_prompts = !isolated && !smoke && !cfg!(test);
        let whats_new = startup_prompts && Self::needs_whats_new(&preferences, preferences_existed, &whats_new_notes);
        crate::i18n::set_language(crate::i18n::resolve(&preferences.language));
        #[cfg(test)]
        if isolated && preferences.language.is_empty() { crate::i18n::set_language("fr"); }
        if std::env::args().any(|a| a == "--light") {
            preferences.dark = false;
        }
        theme::set_theme(&preferences.scheme, &preferences.accent);
        theme::apply(ctx, preferences.dark);
        let (tx, events) = mpsc::channel();
        let device = if isolated {
            mpsc::channel().0
        } else {
            device::spawn(tx.clone(), path.clone())
        };
        let desktop = if isolated || smoke {
            None
        } else {
            crate::audio::spawn(tx.clone());
            Some(crate::desktop::spawn(tx.clone(), ctx.clone()))
        };
        let state_monitor = if isolated || smoke { mpsc::channel().0 } else { crate::state_display::spawn(tx.clone()) };
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
            state_monitor, state_generation: 0, state_values: [None; 32], device_model: None,
            gate,
            long_press: LongPress::default(),
            long_editor: false,
            wanted: false,
            status: tr("Aucun périphérique").into(),
            logs: VecDeque::new(),
            started: Instant::now(),
            last_edit: Instant::now(),
            show_logs: false,
            settings: false,
            show_device_report: false,
            hardware_report: None,
            show_hardware_test: false,
            hardware_test: None,
            show_saved_variables: false,
            show_about: false,
            crop_session: None,
            ffmpeg_offer: None,
            ffmpeg_installing: false,
            update_result: None,
            update_rx: None,
            update_phase: None,
            update_confirm: None,
            update_installed: crate::app_update::installed(),
            startup_prompts,
            startup_checked: false,
            startup_rx: None,
            startup_offer: None,
            whats_new,
            whats_new_notes,
            rename: false,
            notice: tr("Tout est prêt.").into(),
            error: false,
            smoke,
            frames: 0,
            capturing: false,
            last_key: None,
            key_previews: key_preview::Cache::default(),
            plugin_faces: HashMap::new(),
            plugin_errors: HashMap::new(),
            plugin_external_effects: isolated.then(Vec::new),
            plugin_report_cache: HashMap::new(),
            plugin_oracle_cache: HashMap::new(),
            plugin_real_confirmation: None,
            plugin_inventory: None,
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
            import_reports: Vec::new(),
            dialog_tx,
            dialog_rx,
            dialog_pending: false,
            page_manager: false,
            delete_page: None,
            desktop,
            audio: Default::default(),
            obs_state: Default::default(),
            running_block: vec![],
            workshop_journal: Default::default(),
            open_requested: false,
            hide_frames: if !cfg!(test) && std::env::args().any(|a| a == "--background") { 3 } else { 0 },
            tray_ready: false,
            quitting: false,
            show_workflow: false,
            workshop_chat: Default::default(),
            local_ai: Default::default(),
            local_ai_check: None,
            local_ai_install: None,
            local_ai_status: String::new(),
            assistant_tab: 0,
            settings_tab: 0,
            show_assets: false,
            show_desktop_settings: false,
            available_icons: vec![],
            available_plugins: vec![],
            icon_search: String::new(),
            icon_pack: String::new(),
            asset_tab: 0,
            trigger_recording: false,
            trigger_kind: "app_open".into(),
            trigger_value: "obs64.exe".into(),
            trigger_days: vec![],
            trigger_minutes: 5,
            trigger_input: false,
            icon_target: None,
            icon_selected: None,
            icon_notice: String::new(),
            assistant_request: String::new(),
            flicker: None, flicker_path: None, diagnostic_deck: None, diagnostic_inspector: None, diagnostic_key_width: 0.0,
        };
        if let Some(desktop) = &app.desktop {
            let _ = desktop.tx.send(crate::desktop::Command::Triggers(app.preferences.triggers.clone()));
        }
        if let Some(e) = load_error {
            app.fail(e);
        } else if (!ui_test && !smoke) || (!isolated && std::env::args().any(|a| a == "--connect"))
        {
            app.connect();
        }
        if !preferences_valid {
            app.fail(tr("Préférences illisibles : le fichier est conservé. Restaure preferences.json.bak avant d’enregistrer Mes binds.").into());
        }
        if app.startup_prompts && !app.whats_new && app.preferences.last_seen_version != env!("CARGO_PKG_VERSION") {
            app.preferences.last_seen_version = env!("CARGO_PKG_VERSION").into();
            app.save_preferences();
        }
        app.refresh_hotkeys();
        app.sync_state_display();
        app
    }
    fn needs_whats_new(preferences: &Preferences, existed: bool, notes: &str) -> bool {
        preferences.last_seen_version != env!("CARGO_PKG_VERSION")
            && !(preferences.last_seen_version.is_empty() && !existed)
            && !crate::app_update::release_notes(notes, crate::i18n::resolve(&preferences.language)).is_empty()
    }
    fn start_update_check(&mut self, ctx: &egui::Context, startup: bool) {
        if startup {
            if self.startup_checked { return; }
            self.startup_checked = true;
            let (tx, rx) = mpsc::channel();
            self.startup_rx = Some(rx);
            let context = ctx.clone();
            std::thread::spawn(move || {
                let result = crate::app_update::check().map_err(|e| e.to_string());
                let _ = tx.send(result);
                context.request_repaint();
            });
        } else {
            self.update_result = None;
            self.update_phase = Some(tr("Vérification en cours…"));
            let (tx, rx) = mpsc::channel();
            self.update_rx = Some(rx);
            let context = ctx.clone();
            std::thread::spawn(move || {
                let result = crate::app_update::check().map_err(|e| trf("Vérification impossible : {error}", &[("error", &e)]));
                let _ = tx.send(about::UpdateEvent::Checked(result));
                context.request_repaint();
            });
        }
    }
    fn open_hardware_report(&mut self) {
        self.hardware_report = Some(crate::hardware_report::collect(
            &self.path, &self.status, &self.device_label(), self.gate.connected, &self.logs));
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
    fn sync_state_display(&mut self) {
        self.state_generation = self.state_generation.wrapping_add(1);
        self.state_values = [None; 32];
        let _ = self.state_monitor.send(crate::state_display::Command::Page(
            self.state_generation, self.config.keys().clone()));
    }
    fn state_key(&self, index: usize, key: Key) -> Key {
        match (key.state_display.clone(), self.state_values.get(index).copied().flatten()) {
            (Some(display), Some(active)) => display.apply(&key, active),
            _ => key,
        }
    }
    fn send_state_face(&self, index: usize) {
        if !self.wanted || !self.gate.connected || self.gate.dirty || self.pending.is_some()
            || self.sleep.sleeping || index >= self.device_keys() || index >= self.applied.keys().len() { return; }
        let key = self.state_key(index, self.applied.keys()[index].clone());
        if let Ok(font) = crate::key_render::font() {
            if let Ok(jpeg) = crate::key_render::for_plugin(&key, self.applied.image_size, &font, None, None) {
                let _ = self.device.send(Command::KeyImage(index, jpeg));
            }
        }
    }
    fn edited(&mut self) {
        self.sync_state_display();
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
        self.sync_state_display();
        let command = if connect {
            Command::Connect(deployed.clone(), revision)
        } else {
            Command::Update(deployed, revision)
        };
        if self.device.send(command).is_err() {
            self.fail(tr("Le service USB s’est arrêté. Relance NovyDeck.").into());
            self.gate.disconnect();
        }
    }
    fn connect(&mut self) {
        if self.gate.invalid {
            return;
        }
        if let Err(e) = self.config.validate_structure() {
            self.fail(trf("Configuration à corriger : {error}", &[("error", &e)]));
            return;
        }
        if self.gate.dirty {
            if let Err(e) = store::save(&self.path, &self.config) {
                self.fail(trf("Sauvegarde impossible : {error}", &[("error", &e)]));
                return;
            }
        }
        self.wanted = true;
        self.upload(true);
        self.log(tr("Connexion au périphérique…").into());
    }
    fn save(&mut self) {
        if self.gate.invalid {
            self.fail(tr("Importe une configuration valide avant d’enregistrer.").into());
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
                    trf("Modifications enregistrées, envoi au {device}…", &[("device", &self.device_name())])
                } else {
                    tr("Brouillons enregistrés. Synchronisation des touches valides…").into()
                });
            }
            Err(e) => self.fail(trf("Sauvegarde impossible : {error}", &[("error", &e)])),
        }
    }
    fn execute_key(&mut self, key: Key, dry: bool, delayed: bool, config: &Config) {
        if let Err(e) = validate_key(&key, config) {
            self.fail(trf("Action à corriger : {error}", &[("error", &e)]));
            return;
        }
        if key.steps.is_empty() {
            self.log(tr("Cette touche n’a pas encore d’action. Ajoute une action depuis la bibliothèque.").into());
            return;
        }
        self.engine.start(key, dry, if delayed { 3000 } else { 0 }, config.obs.clone());
    }
    fn grid(&self) -> DeviceGrid {
        self.device_model.clone().unwrap_or_else(DeviceGrid::default)
    }
    fn device_keys(&self) -> usize { self.grid().keys }
    /// Name of the connected model, or a neutral word when nothing is plugged in.
    fn device_name(&self) -> String {
        self.device_model.as_ref().map_or_else(|| tr("périphérique").into(), |d| d.name.clone())
    }
    fn is_d6(&self) -> bool { self.device_model.as_ref().is_none_or(|d| d.name.contains("D6")) }
    /// Animations d'interface actives (réglage « Réduire les animations »).
    fn animations(&self) -> bool {
        !self.preferences.reduce_motion
    }
    /// État résumé des deux IA (locale et connectée) pour un seul bouton.
    fn ai_status_label(&self) -> &'static str {
        let connected = self.assistant.as_ref().is_some_and(|s| s.enabled());
        match (self.preferences.local_ai.enabled, connected) {
            (true, true) => tr("IA : locale + connectée"),
            (true, false) => tr("IA : locale"),
            (false, true) => tr("IA : connectée"),
            (false, false) if self.preferences.allow_connected_ai => tr("IA : en attente"),
            _ => tr("IA : désactivée"),
        }
    }
    fn device_label(&self) -> String {
        self.device_model.as_ref().map_or_else(|| tr("Aucun périphérique").into(), |d| {
            trf("{name} · {keys} touches{experimental}", &[("name", &d.name), ("keys", &d.keys), ("experimental", &if d.experimental { tr(" · expérimental") } else { "" })])
        })
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
                Event::Log(s) => { self.workshop_journal.log(&s); self.log(s); },
                Event::Notification(title, message) => {
                    self.log(format!("Notification — {title} : {message}"));
                    if self.tray_ready {
                        if let Some(desktop) = &self.desktop { let _ = desktop.tx.send(crate::desktop::Command::Notify(title, message)); }
                    }
                }
                Event::Block(path) => self.running_block = path,
                Event::WorkshopStep { path, name, elapsed } => self.workshop_journal.step(path, name, elapsed),
                Event::WorkshopFinished { ok, elapsed, message } => self.workshop_journal.finish(ok, elapsed, message),
                Event::D6Brightness(mode, value) => {
                    self.config.brightness = crate::model::d6_brightness(self.config.brightness, &mode, value);
                    if self.wanted && self.gate.connected && !self.sleep.sleeping {
                        let _ = self.device.send(Command::Brightness(self.config.brightness));
                    }
                    self.edited();
                }
                Event::StateDisplay(generation, index, active) => {
                    if generation == self.state_generation && index < 15 {
                        self.state_values[index] = Some(active);
                        self.send_state_face(index);
                    }
                }
                Event::ToggleVisual => { if self.wanted && self.gate.connected { self.upload(false); } },
                Event::D6Sleep => {
                    self.sleep.sleeping = true;
                    if self.wanted && self.gate.connected { let _ = self.device.send(Command::Brightness(self.config.sleep_brightness)); }
                }
                Event::Foreground(exe) => { self.foreground = exe; self.update_auto_profile(); }
                Event::Open => { self.open_requested = true; self.hide_frames = 0; }
                Event::TrayReady(ready) => {
                    self.tray_ready = ready;
                    if !ready {
                        self.open_requested = true;
                        self.hide_frames = 0;
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
                Event::Automatic(trigger, at) => {
                    if at.elapsed() < Duration::from_secs(3) && self.gate.enabled
                        && !self.gate.invalid && !self.gate.dirty && !self.trigger_recording
                        && self.recording.is_none() && self.engine.allows_global_press(at)
                        && self.preferences.triggers.contains(&trigger)
                    {
                        if let Some(profile) = self.config.profiles.iter().find(|p| p.name == trigger.profile) {
                            if let Some(key) = profile.keys_at(&trigger.page).and_then(|keys| keys.get(trigger.key)) {
                                let mut key = key.clone();
                                key.runtime_id = Some(crate::engine::identity(&trigger.profile, &trigger.page, trigger.key));
                                self.log(trf("Déclencheur : {trigger} → touche {key}", &[("trigger", &trigger.description()), ("key", &(trigger.key + 1))]));
                                let config = self.config.clone();
                                self.execute_key(key, false, false, &config);
                            }
                        }
                    }
                }
                Event::Hotkey(index, at) => {
                    if self.engine.allows_global_press(at)
                        && index < self.device_keys()
                        && index < self.applied.keys().len()
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
                Event::HardwareTestPress(raw, index) => {
                    if let Some(test)=&mut self.hardware_test {
                        test.press(&raw,index);
                        let next=test.next;
                        let finished=test.next == test.keys;
                        self.log(trf("Test : brut [{raw}] ; décodé {decoded} ; attendu {expected}", &[("raw", &raw), ("decoded", &(index+1)), ("expected", &next)]));
                        if finished { let _=self.device.send(Command::TestBrightness); }
                    }
                }
                Event::HardwareTestStatus(message) => {
                    if let Some(test)=&mut self.hardware_test {
                        if message.starts_with("Images numérotées") { test.image=message.clone(); }
                        if message.starts_with("Luminosité") || message.starts_with("ÉCHEC luminosité") { test.brightness=message.clone(); }
                        if message.starts_with("ÉCHEC") || message.starts_with("Images numérotées : ÉCHEC") || message.starts_with("Test arrêté") { test.active=false; }
                    }
                    self.log(trf("Assistant périphérique : {message}", &[("message", &message)]));
                }
                Event::DeviceModel { name, keys, rows, cols, experimental } => {
                    if let Some(grid) = DeviceGrid::new(name.clone(), keys, rows, cols, experimental) {
                        crate::model::extend_for_device(&mut self.config, grid.keys);
                        crate::model::extend_for_device(&mut self.applied, grid.keys);
                        self.selected = self.selected.min(grid.keys - 1);
                        if let Some(plugins)=&self.plugins {plugins.device(crate::plugin_runtime::Device::from_grid(grid.name.clone(),grid.cols,grid.rows));}
                        self.device_model = Some(grid);
                        self.sync_state_display();
                    } else {
                        self.fail(trf("Grille HID invalide : {name}, {keys} touches ({rows} × {cols})", &[("name", &name), ("keys", &keys), ("rows", &rows), ("cols", &cols)]));
                    }
                    self.log(trf("Modèle HID détecté : {name} · {keys} touches ({rows} × {cols}){verification}",
                        &[("name", &name), ("keys", &keys), ("rows", &rows), ("cols", &cols), ("verification", &if experimental { tr(" · EXPÉRIMENTAL, non vérifié sur matériel") } else { tr(" · vérifié") })]));
                }
                Event::Device(connected, message) => {
                    if self.status != message {
                        self.log(message.clone());
                    }
                    self.status = message;
                    if !connected {
                        if let Some(test)=&mut self.hardware_test { test.active=false; test.brightness=tr("ÉCHEC : périphérique déconnecté").into(); }
                        if let Some(plugins)=&self.plugins {plugins.device(crate::plugin_runtime::Device::virtual_device());}
                        self.device_model = None;
                        self.selected = self.selected.min(14);
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
                                for index in 0..self.device_keys() { if self.state_values[index].is_some() { self.send_state_face(index); } }
                                if let Some(plugins)=&mut self.plugins { plugins.sync(&self.applied); }
                                if self.sleep.sleeping {
                                    let _ = self.device.send(Command::Brightness(self.config.sleep_brightness));
                                }
                            }
                        }
                        self.log(
                            trf("{device} synchronisé. Les touches suivent le mode affiché en haut.", &[("device", &self.device_name())]),
                        );
                    }
                }
                Event::Key(index, at) => {
                    if self.hardware_test.as_ref().is_some_and(|t| t.active) { continue; }
                    let wake = self.sleep.press(Instant::now());
                    if wake { self.wake_display(); self.long_press.clear(); continue; }
                    self.last_key = Some((index, Instant::now()));
                    match if self.trigger_recording {
                        Decision::Blocked(tr("Enregistrement du raccourci en cours"))
                    } else {
                        self.gate.decision_for_age(at.elapsed())
                    } {
                        Decision::Inspect => {
                            self.selected = index;
                            self.log(trf("Diagnostic : touche {key} reçue, aucune action exécutée.", &[("key", &format!("{:02}", index + 1))]));
                        }
                        Decision::Blocked(reason) => {
                            self.log(trf("Touche {key} : {reason}", &[("key", &format!("{:02}", index + 1)), ("reason", &reason)]))
                        }
                        Decision::Execute => {
                            if index < self.device_keys() && index < self.applied.keys().len() && self.applied.keys()[index].plugin.is_some() { if let Some(plugins)=&self.plugins { plugins.key(index,true); } }
                            let has_long = !self.applied.keys()[index].long_steps.is_empty();
                            if self.long_press.press(index, at, has_long) {
                                self.execute_physical(index, false);
                            }
                        }
                    }
                }
                Event::KeyRelease(index, at) => {
                    if self.hardware_test.as_ref().is_some_and(|t| t.active) { continue; }
                    if index < self.device_keys() && index < self.applied.keys().len() {
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
            Err(e) => self.fail(trf("Navigation impossible : {error}", &[("error", &e)])),
        }
    }
    fn create_page(&mut self, folder: bool) {
        if self.config.profiles[self.config.active].pages.len() >= 128 {
            self.fail(tr("128 pages et dossiers maximum par profil.").into());
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
        self.log(tr("Page supprimée et touches de navigation correspondantes retirées.").into());
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
                        self.log(trf("Raccourci enregistré : {text}", &[("text", &text)]));
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
    /// Ajoute (ou réinitialise) le profil de démonstration, sans toucher aux autres profils.
    pub(super) fn add_demo_profile(&mut self) {
        let dir = self.path.parent().unwrap().join("assets").join("demo");
        let profile = match crate::demo_profile::write_media(&dir) {
            Ok((photo, animation)) => crate::demo_profile::profile(&photo, &animation),
            Err(e) => { self.fail(trf("Profil de démonstration non créé : {error}", &[("error", &e)])); return; }
        };
        let index = match self.config.profiles.iter().position(|p| p.name == crate::demo_profile::NAME) {
            Some(i) => { self.config.profiles[i] = profile; i }
            None if self.config.profiles.len() < 64 => { self.config.profiles.push(profile); self.config.profiles.len() - 1 }
            None => { self.fail(tr("64 profils au maximum : supprime un profil pour ajouter la démonstration.").into()); return; }
        };
        self.config.active = index;
        self.base_profile = index;
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
                .add_filter(
                    tr("Configuration NovyDeck ou sauvegarde Stream Deck"),
                    &[
                        "json",
                        "bak",
                        "StreamDeckProfilesBackup",
                        "streamdeckprofilesbackup",
                        "streamDeckProfile",
                        "streamdeckprofile",
                    ],
                )
                .add_filter(tr("Configuration NovyDeck (*.json, *.bak)"), &["json", "bak"])
                .add_filter(
                    tr("Sauvegarde Stream Deck (*.StreamDeckProfilesBackup)"),
                    &["StreamDeckProfilesBackup", "streamdeckprofilesbackup"],
                )
                .add_filter(
                    tr("Profil Stream Deck (*.streamDeckProfile)"),
                    &["streamDeckProfile", "streamdeckprofile"],
                )
                .pick_file()?;
            let is_streamdeck = path
                .extension()
                .map(|ext| {
                    let s = ext.to_string_lossy().to_ascii_lowercase();
                    s.contains("streamdeck")
                })
                .unwrap_or(false);
            if is_streamdeck {
                let result = crate::elgato::load_all(&path);
                return Some(Box::new(move |app| match result {
                    Ok(reports) => app.import_reports = reports,
                    Err(e) => app.fail(trf("Import Stream Deck impossible : {error}", &[("error", &e)])),
                }));
            }
            let loaded = store::load(&path);
            Some(Box::new(move |app| match loaded {
                Ok(c) => {
                    app.config = c;
                    app.base_profile = app.config.active;
                    app.auto_active = false;
                    app.selected = 0;
                    app.gate.invalid = false;
                    app.edited();
                    app.log(tr("Profils importés. Les touches restent en pause jusqu'à leur réactivation.").into());
                }
                Err(e) => {
                    if let Ok(reports) = crate::elgato::load_all(&path) {
                        app.import_reports = reports;
                    } else {
                        app.fail(trf("Import impossible : {error}", &[("error", &e)]));
                    }
                }
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
                Ok(()) => app.log(tr("Profils exportés. Réactive les touches quand tu es prêt.").into()),
                Err(e) => app.fail(trf("Export impossible : {error}", &[("error", &e)])),
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
            self.fail(tr("Mode HTTP arrêté. Vérifie que le port local 9846 est disponible.").into());
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
        if matches!(name, "deckforge_configure_key" | "deckforge_simulate_macro" | "deckforge_request_run") {
            let key = args["key"].as_u64().ok_or("Numéro de touche requis")?;
            if key == 0 || key > self.device_keys() as u64 {
                return Err(format!("Touche hors grille : 1 à {} pour le modèle actif", self.device_keys()));
            }
        }
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
        self.log(trf("Assistant IA : {name}", &[("name", &name)]));
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
            "deckforge_get_state" => {
                let scope = (self.config.active, self.config.page.clone(), self.selected);
                let pending_request = workshop_chat_ui::mcp_request(&mut self.workshop_chat, &scope);
                let mut state = json!({"connected":self.gate.connected,"physical_keys":format!("{:?}",self.gate.decision()),"current_profile":self.config.profiles[self.config.active].name,"current_page":self.config.page,"trail":self.config.trail,"profiles":self.config.profiles.iter().map(|p|json!({"name":p.name,"pages":p.pages.iter().map(|page|json!({"id":page.id,"name":page.name,"folder":page.folder})).collect::<Vec<_>>(),"keys":p.keys.iter().enumerate().map(|(i,k)|json!({"key":i+1,"label":k.label,"steps":k.steps.len()})).collect::<Vec<_>>()})).collect::<Vec<_>>(),"pending_proposals":self.proposals.pending.len(),"configuration_invalid":self.gate.invalid});
                if let Some(request) = pending_request { state["pending_request"] = request; }
                Ok(state)
            },
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
                    return Err("Récupère d’abord la configuration dans NovyDeck".into());
                }
                let result = self.proposals.propose(name, args, &self.config);
                if result.is_ok() && self.workshop_chat.awaiting.take().is_some() {
                    self.workshop_chat.say(tr("Proposition de ton IA reçue. Vérifie-la dans la fenêtre de validation."));
                }
                result
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
            self.fail(tr("Active d’abord Assistant IA.").into());
            return;
        }
        if self.http_child.is_some() {
            return;
        }
        match std::process::Command::new(self.path.with_file_name("novydeck-mcp.exe"))
            .args(["--http", "127.0.0.1:9846"])
            .creation_flags(0x08000000)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => {
                self.http_child = Some(child);
                self.log(tr("Mode MCP HTTP local activé sur le port 9846.").into());
            }
            Err(e) => self.fail(trf("Mode HTTP indisponible : {error}", &[("error", &e)])),
        }
    }
    fn stop_http(&mut self) {
        if let Some(mut child) = self.http_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    fn connect_ai_window(&mut self, ctx: &egui::Context) {
        if !self.connect_window { return; }
        let mut open=true;
        egui::Window::new(tr("Connecter une IA")).open(&mut open).default_width(530.0).resizable(true).vscroll(true).show(ctx,|ui|{
            ui.label(tr("Choisis ton application IA. NovyDeck n'écrit jamais de clé dans ses réglages."));
            let locations=crate::mcp_connect::Locations::system();
            let exe=crate::mcp_connect::connector(&self.path);
            for client in [crate::mcp_connect::Client::Desktop,crate::mcp_connect::Client::Code,crate::mcp_connect::Client::Codex,crate::mcp_connect::Client::Cursor] {
                egui::Frame::group(ui.style()).show(ui,|ui|{
                    ui.label(RichText::new(client.name()).strong().size(17.0));
                    let configured=locations.as_ref().is_ok_and(|loc|crate::mcp_connect::configured(client,loc));
                    ui.label(if configured {tr("Configuré sur ce PC")} else {tr("Pas encore configuré")});
                    ui.horizontal(|ui|{
                        if ui.add_enabled(exe.is_ok() && locations.is_ok(),egui::Button::new(tr("Configurer automatiquement"))).clicked() {
                            match if client==crate::mcp_connect::Client::Code {crate::mcp_connect::configure_code(locations.as_ref().unwrap(),exe.as_ref().unwrap(),false)} else {crate::mcp_connect::configure(client,locations.as_ref().unwrap(),exe.as_ref().unwrap(),false)}{Ok(msg)=>self.log(msg),Err(err)=>self.fail(err)}
                        }
                        if ui.add_enabled(configured,egui::Button::new(tr("Retirer"))).clicked() {
                            match if client==crate::mcp_connect::Client::Code {crate::mcp_connect::configure_code(locations.as_ref().unwrap(),&self.path,true)} else {crate::mcp_connect::configure(client,locations.as_ref().unwrap(),&self.path,true)}{Ok(msg)=>self.log(msg),Err(err)=>self.fail(err)}
                        }
                    });
                });ui.add_space(8.0);
            }
            if let Err(e)=&exe { ui.colored_label(Color32::DARK_RED,e); }
            egui::Frame::group(ui.style()).show(ui,|ui|{
                ui.label(RichText::new(tr("ChatGPT : Secure MCP Tunnel")).strong());
                ui.label(tr("1. Ouvre la documentation du tunnel. 2. Copie le chemin du connecteur stdio à fournir au tunnel. 3. Dans ChatGPT : Plugins, Ajouter un serveur MCP, puis Tunnel. Disponibilité à vérifier selon ton compte. Le port HTTP reste local."));
                if ui.button(tr("Ouvrir la page du tunnel")).clicked(){let _={ui.ctx().open_url(egui::OpenUrl::new_tab("https://developers.openai.com/api/docs/guides/secure-mcp-tunnels"));Ok::<(),String>(())};}
                if ui.button(tr("Copier la commande stdio")).clicked(){ui.ctx().copy_text(self.path.with_file_name("novydeck-mcp.exe").to_string_lossy().into_owned());}
            });
            if ui.button(tr("Autre client : copier le JSON standard")).clicked(){
                let config=serde_json::json!({"mcpServers":{"deckforge":{"command":self.path.with_file_name("novydeck-mcp.exe").to_string_lossy(),"args":[]}}});
                ui.ctx().copy_text(serde_json::to_string_pretty(&config).unwrap());
            }
            if ui.button(tr("Tester la connexion")).clicked(){
                let result=if !self.assistant.as_ref().is_some_and(|s|s.enabled()) {Err(tr("Assistant désactivé : active d'abord Autoriser les IA connectées").into())} else {crate::mcp_connect::test_connection(&self.path)};
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
            egui::Window::new(tr("Assistant IA")).open(&mut open).collapsible(false).resizable(true).default_width(510.0).vscroll(true).max_height((ctx.screen_rect().height()-100.0).max(400.0)).show(ctx,|ui|{
            ui.horizontal(|ui| {
                for (i, (ico, label)) in [(Ico::Brain, tr("IA locale (sur ce PC)")), (Ico::Connection, tr("IA connectée (Claude, ChatGPT…)"))].into_iter().enumerate() {
                    if iconset::tab(ui, self.assistant_tab == i as u8, ico, RichText::new(label).size(15.0)).clicked() { self.assistant_tab = i as u8; }
                }
            });
            ui.separator();
            motion::fade(ui, "assistant_tab", self.assistant_tab, 0.15, self.animations());
            if self.assistant_tab == 0 {
                ui.label(RichText::new(tr("IA locale de l'assistant (facultative)")).size(18.0).strong());
                self.local_ai_settings(ui, &p);
                return;
            }
            ui.label(RichText::new(tr("Prépare tes macros avec ton IA connectée.")).size(18.0).strong());ui.add_space(6.0);
            let mut enabled=self.assistant.as_ref().is_some_and(|s|s.enabled());
            ui.horizontal(|ui|{if theme::toggle(ui,&mut enabled).changed(){if let Some(server)=&self.assistant{server.set_enabled(enabled);}if !enabled{self.stop_http();self.proposals.reject_all();self.assistant_cache.clear();self.workshop_chat.awaiting=None;}self.preferences.allow_connected_ai=enabled;self.save_preferences();self.log(if enabled{tr("Connecteur MCP activé.")}else{tr("Connecteur MCP désactivé.")}.into());}ui.label(RichText::new(if enabled{tr("IA connectée activée")}else{tr("IA connectée désactivée")}).strong());});
            ui.add_space(10.0);ui.label(tr("Ton assistant peut lire les profils et proposer des actions. Les changements et les exécutions réelles attendent toujours ta validation ici."));ui.add_space(8.0);
            ui.separator();ui.label(RichText::new(tr("Décris l’automatisation souhaitée")).strong());
            ui.add(egui::TextEdit::multiline(&mut self.assistant_request).desired_rows(3).desired_width(f32::INFINITY).hint_text(tr("Ex. : basculer entre mon casque et mes enceintes, avec une icône pour chaque état.")));
            ui.label(RichText::new(trf("Destination : {profile} · touche {key}", &[("profile", &self.config.profiles[self.config.active].name), ("key", &format!("{:02}", self.selected+1))])).small());
            if ui.add_enabled(!self.assistant_request.trim().is_empty(),egui::Button::new(tr("Copier la demande pour mon IA"))).clicked(){ui.ctx().copy_text(self.automation_prompt());self.log(tr("Demande copiée. Colle-la dans ton IA connectée au MCP NovyDeck.").into());}
            ui.label(RichText::new(tr("Ton IA doit être connectée au MCP. Elle proposera les blocs ici ; tu pourras les valider puis les modifier dans l’Atelier. Aucun modèle local ni appel d’IA n’est lancé par ce champ.")).small().color(p.muted));
            ui.separator();
            if self.assistant.is_none(){ui.colored_label(Color32::DARK_RED,tr("Le connecteur local n’a pas pu démarrer."));}
            ui.label(RichText::new(tr("Connexion locale MCP · Claude, Codex et clients compatibles stdio. Aucune clé d’IA à saisir dans NovyDeck.")).small().color(p.muted));
            if ui.button(tr("Connecter une IA")).clicked(){self.connect_window=true;}
            if ui.button(tr("Copier la configuration MCP")).clicked(){let exe=self.path.with_file_name("novydeck-mcp.exe");let config=serde_json::json!({"mcpServers":{"deckforge":{"command":exe.to_string_lossy(),"args":[]}}});ui.ctx().copy_text(serde_json::to_string_pretty(&config).unwrap());self.log(tr("Configuration MCP copiée.").into());}
            ui.label(RichText::new(tr("Le guide MCP fourni avec l’application détaille la connexion à ChatGPT et aux autres clients.")).small().color(p.muted));
            ui.add_space(14.0);ui.separator();ui.label(RichText::new(tr("ChatGPT web / clients HTTP")).strong());
            ui.label(RichText::new(tr("Le mode HTTP reste local au PC. Un tunnel ou un proxy compatible est nécessaire pour un service IA distant.")).small().color(p.muted));
            ui.horizontal(|ui|{if ui.add_enabled(enabled,egui::Button::new(if self.http_child.is_some(){tr("Arrêter HTTP")}else{tr("Démarrer HTTP")})).clicked(){if self.http_child.is_some(){self.stop_http();}else{self.start_http();}}if self.http_child.is_some()&&ui.button(tr("Copier l’adresse")).clicked(){ui.ctx().copy_text("http://127.0.0.1:9846/mcp".into());}});
            if self.http_child.is_some()&&ui.button(tr("Copier la clé HTTP")).clicked(){match std::fs::read_to_string(self.path.with_file_name("mcp-http-token.txt")){Ok(t)=>{ui.ctx().copy_text(t.trim().into());self.log(tr("Clé HTTP copiée. Utilise-la uniquement dans ton client MCP.").into());},Err(_)=>self.fail(tr("La clé n’est pas encore prête. Réessaie dans un instant.").into())}}
        });
            self.assistant_settings = open;
        }
        self.connect_ai_window(ctx);
        let mut decision = None;
        if let Some(p) = self.proposals.pending.front() {
            egui::Window::new(tr("Proposition de ton assistant")).id(egui::Id::new("ai_review")).collapsible(false).resizable(true).default_width(570.0).show(ctx,|ui|{
                theme::pill(ui,tr("VALIDATION NÉCESSAIRE"),accent());ui.add_space(8.0);ui.label(RichText::new(&p.title).size(18.0).strong());
                ui.label(RichText::new(tr("Aucun changement ni lancement n’a encore eu lieu. Vérifie les actions avant de continuer.")).color(theme::palette(ctx).muted));
                egui::ScrollArea::vertical().max_height(360.0).show(ui,|ui|{ui.label(&p.details);});ui.add_space(12.0);
                ui.horizontal(|ui|{if ui.button(tr("Refuser")).clicked(){decision=Some(false);}if ui.add(theme::primary(RichText::new(tr("Valider cette proposition")).color(Color32::WHITE))).clicked(){decision=Some(true);}});
            });
        }
        if let Some(accept) = decision {
            self.workshop_chat.awaiting = None;
            match self.proposals.decide(accept, &self.config) {
                Ok(Some(crate::assistant::Change::Config(c))) => {
                    self.engine.stop();
                    self.config = c;
                    self.selected = 0;
                    self.edited();
                    self.log(
                        tr("Proposition appliquée. Sauvegarde et synchronisation en cours.").into(),
                    );
                }
                Ok(Some(crate::assistant::Change::Run(key))) => {
                    let c = self.config.clone();
                    self.execute_key(key, false, true, &c);
                }
                Ok(None) => self.log(tr("Proposition refusée.").into()),
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
                                theme::section_title(ui, tr("PROFILS"));
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.menu_button("•••", |ui| {
                                        if ui
                                            .add_enabled(
                                                self.config.profiles.len() < 64,
                                                egui::Button::new(tr("Nouveau profil")),
                                            )
                                            .clicked()
                                        {
                                            self.add_profile(false);
                                            ui.close_menu();
                                        }
                                        if ui.button(tr("Renommer / dupliquer")).clicked() {
                                            self.rename = true;
                                            ui.close_menu();
                                        }
                                        if ui.button(tr("Applications associées")).clicked() {
                                            self.show_apps = true;
                                            ui.close_menu();
                                        }
                                        if ui.button(tr("Profil de démonstration (15 touches)"))
                                            .on_hover_text(tr("Ajoute ou réinitialise « NovyDeck Demo / README ». Tes autres profils ne sont pas modifiés."))
                                            .clicked()
                                        {
                                            self.add_demo_profile();
                                            ui.close_menu();
                                        }
                                    });
                                    if ui
                                        .add_enabled(
                                            self.config.profiles.len() < 64,
                                            egui::Button::new("+").small(),
                                        )
                                        .on_hover_text(tr("Nouveau profil"))
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
                            let icon_color = if selected { accent() } else { p.muted };
                            let text_color = if selected { p.ink } else { p.muted };
                            let icon_pos = if compact {
                                r.center()
                            } else {
                                r.left_center() + Vec2::new(16.0, 0.0)
                            };
                            let _ = icon_color;
                            iconset::paint(ui.painter(), icon_pos, 18.0, Ico::Profile, Family::Accent, if selected { 1.0 } else { 0.6 }, 15);
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
                                theme::section_title(ui, tr("PAGES"));
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.menu_button("+", |ui| {
                                        if ui.button(tr("Nouvelle page")).clicked() {
                                            self.create_page(false);
                                            ui.close_menu();
                                        }
                                        if ui.button(tr("Gérer pages et dossiers…")).clicked() {
                                            self.page_manager = true;
                                            ui.close_menu();
                                        }
                                        if ui
                                            .button(tr("Dossier sur la touche sélectionnée"))
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
                            let dot_color = if selected { accent() } else { p.muted };
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
                    (tr("Connecté"), GREEN)
                } else if self.wanted {
                    (tr("En attente"), theme::WARNING)
                } else {
                    (tr("Déconnecté"), p.muted)
                };
                let (card_r, card_resp) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), footer_h - 4.0),
                    egui::Sense::click(),
                );
                let card_resp = if !self.gate.connected && !self.wanted {
                    card_resp.on_hover_text(tr("Cliquer pour connecter le périphérique"))
                } else {
                    card_resp
                };
                if !self.gate.connected && !self.wanted && card_resp.clicked() {
                    self.connect();
                }
                ui.painter().rect_filled(card_r, 10.0, p.card);
                if compact {
                    motion::status_dot(ui.painter(), card_r.center() - Vec2::new(0.0, 7.0), status_color, self.wanted && !self.gate.connected, self.animations());
                    ui.painter().text(
                        card_r.center() + Vec2::new(0.0, 8.0),
                        egui::Align2::CENTER_CENTER,
                        "Deck",
                        egui::FontId::proportional(10.0),
                        p.ink,
                    );
                    card_resp.on_hover_text(format!("{} · {status_label}", self.device_label()));
                } else {
                    ui.painter().text(
                        card_r.min + Vec2::new(12.0, 10.0),
                        egui::Align2::LEFT_TOP,
                        self.device_label(),
                        egui::FontId::proportional(11.0),
                        p.ink,
                    );
                    let dot_center = card_r.min + Vec2::new(16.0, 36.0);
                    motion::status_dot(ui.painter(), dot_center, status_color, self.wanted && !self.gate.connected, self.animations());
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
        if !self.show_workflow { self.header_standard(ctx); return; }
        let p = theme::palette(ctx);
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .inner_margin(egui::Margin::symmetric(18, 10)),
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                    let logo = theme::logo_texture(ctx);
                    ui.add(egui::Image::new((logo.id(), Vec2::splat(34.0))));
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(RichText::new("NovyDeck").size(18.0).strong());
                        ui.label(RichText::new(self.device_label()).size(10.0).color(p.muted));
                    });
                    ui.add_space(12.0);
                    if ui
                        .add_enabled(!self.history.is_empty(), iconset::secondary_button(ctx, Ico::Undo, if ctx.screen_rect().width() < 1320.0 { "" } else { tr("Annuler") }))
                        .on_hover_text(tr("Annuler — Ctrl+Z"))
                        .clicked()
                    {
                        self.undo_edit();
                    }
                    if ui
                        .add_enabled(!self.future.is_empty(), iconset::secondary_button(ctx, Ico::Redo, if ctx.screen_rect().width() < 1320.0 { "" } else { tr("Rétablir") }))
                        .on_hover_text(tr("Rétablir — Ctrl+Y"))
                        .clicked()
                    {
                        self.redo_edit();
                    }
                    });
                    ui.horizontal_wrapped(|ui| {
                        iconset::menu(ui, Ico::Settings, tr("Réglages"), |ui| {
                        if ui
                            .add(iconset::secondary_button(ctx, Ico::Robot, self.ai_status_label()))
                            .on_hover_text(tr("Assistant IA : IA locale sur ce PC et IA connectée (Claude, ChatGPT…)"))
                            .clicked()
                        {
                            self.assistant_settings = true;
                        }
                        if ui
                            .add(iconset::secondary_button(ctx, if self.preferences.dark { Ico::Sun } else { Ico::Moon }, if self.preferences.dark {
                                tr("Thème clair")
                            } else {
                                tr("Thème sombre")
                            }))
                            .on_hover_text(tr("Changer le thème de toute l’interface"))
                            .clicked()
                        {
                            self.preferences.dark = !self.preferences.dark; self.preferences.scheme = theme::current_scheme(self.preferences.dark).into();
                            theme::apply(ctx, self.preferences.dark);
                            self.save_preferences();
                        }

 ui.separator();
                            if ui.add(iconset::button(ctx, Ico::Keyboard, tr("Windows et raccourcis…"))).clicked() {
                                self.show_desktop_settings = true;
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Picture, tr("Icônes et extensions…"))).clicked() {
                                self.reload_icons();
                                self.show_assets = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button(tr("Quitter NovyDeck")).clicked() {
                                self.quitting = true;
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Install, tr("Importer depuis Stream Deck…"))).clicked() {
                                self.import_elgato();
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Variable, tr("Variables enregistrées…"))).clicked() {
                                self.show_saved_variables = true;
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Diagnostic, tr("Tester mon périphérique…"))).clicked() {
                                self.show_hardware_test = true;
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Log, tr("Rapport périphérique…"))).clicked() {
                                self.show_device_report = true;
                                ui.close_menu();
                            }
                            if ui.button(tr("Signaler un problème avec mon deck")).clicked() {
                                self.open_hardware_report();
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Device, trf("Réglages — {device}", &[("device", &self.device_name())]))).clicked() {
                                self.settings = true;
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Info, tr("À propos de NovyDeck"))).clicked() {
                                self.show_about = true;
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Robot, tr("Assistant IA…"))).clicked() {
                                self.assistant_settings = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.add(iconset::button(ctx, Ico::Profile, tr("Importer des profils…"))).clicked() {
                                self.import();
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Save, tr("Exporter les profils…"))).clicked() {
                                self.export();
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui
                                .button(if self.wanted {
                                    tr("Déconnecter le périphérique")
                                } else {
                                    tr("Connecter le périphérique")
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
                        // Visible seulement pendant une exécution (Ctrl + Alt + F12 reste toujours actif).
                        if self.engine.running.load(Ordering::SeqCst)
                            && ui
                            .add(
                                theme::danger_button(
                                    RichText::new(tr("Arrêter les macros"))
                                        .color(theme::DANGER),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(egui::Stroke::new(1.0_f32, theme::DANGER)),
                            )
                            .on_hover_text(tr("Arrête les macros en cours et met les touches en pause (Ctrl + Alt + F12). Les applications déjà ouvertes restent ouvertes."))
                            .clicked()
                        {
                            self.engine.stop();
                            self.gate.enabled = false;
                            self.log(tr("Macros arrêtées. Touches en pause.").into());
                        }
                        if theme::toggle(ui, &mut self.gate.enabled).changed() {
                            if !self.gate.enabled {
                                self.engine.stop();
                            } else {
                                self.gate.inspect = false;
                            }
                        }
                        let (label, color) = match self.gate.decision() {
                            Decision::Execute => (tr("Touches actives"), GREEN),
                            Decision::Inspect => ("Diagnostic", accent()),
                            Decision::Blocked(_) => (
                                if !self.gate.enabled {
                                    tr("En pause")
                                } else if !self.gate.connected {
                                    if self.wanted {
                                        tr("En attente")
                                    } else {
                                        tr("Déconnecté")
                                    }
                                } else if self.gate.dirty {
                                    tr("Modifications…")
                                } else {
                                    tr("Synchronisation…")
                                },
                                p.muted,
                            ),
                        };
                        if ctx.screen_rect().width() >= 1200.0 { ui.label(RichText::new(label).size(12.0).strong().color(color)); }
                    });
                });
            });
    }
    fn header_standard(&mut self, ctx: &egui::Context) {
        let p = theme::palette(ctx);
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .inner_margin(egui::Margin::symmetric(18, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let logo = theme::logo_texture(ctx);
                    ui.add(egui::Image::new((logo.id(), Vec2::splat(34.0))));
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(RichText::new("NovyDeck").size(18.0).strong());
                        ui.label(RichText::new(self.device_label()).size(10.0).color(p.muted));
                    });
                    ui.add_space(12.0);
                    if ui
                        .add_enabled(!self.history.is_empty(), iconset::secondary_button(ctx, Ico::Undo, if ctx.screen_rect().width() < 1320.0 { "" } else { tr("Annuler") }))
                        .on_hover_text(tr("Annuler — Ctrl+Z"))
                        .clicked()
                    {
                        self.undo_edit();
                    }
                    if ui
                        .add_enabled(!self.future.is_empty(), iconset::secondary_button(ctx, Ico::Redo, if ctx.screen_rect().width() < 1320.0 { "" } else { tr("Rétablir") }))
                        .on_hover_text(tr("Rétablir — Ctrl+Y"))
                        .clicked()
                    {
                        self.redo_edit();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        iconset::menu(ui, Ico::Settings, tr("Réglages"), |ui| {
                            if ui.add(iconset::button(ctx, Ico::Keyboard, tr("Windows et raccourcis…"))).clicked() {
                                self.show_desktop_settings = true;
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Picture, tr("Icônes et extensions…"))).clicked() {
                                self.reload_icons();
                                self.show_assets = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button(tr("Quitter NovyDeck")).clicked() {
                                self.quitting = true;
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Install, tr("Importer depuis Stream Deck…"))).clicked() {
                                self.import_elgato();
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Variable, tr("Variables enregistrées…"))).clicked() {
                                self.show_saved_variables = true;
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Diagnostic, tr("Tester mon périphérique…"))).clicked() {
                                self.show_hardware_test = true;
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Log, tr("Rapport périphérique…"))).clicked() {
                                self.show_device_report = true;
                                ui.close_menu();
                            }
                            if ui.button(tr("Signaler un problème avec mon deck")).clicked() {
                                self.open_hardware_report();
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Device, trf("Réglages — {device}", &[("device", &self.device_name())]))).clicked() {
                                self.settings = true;
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Info, tr("À propos de NovyDeck"))).clicked() {
                                self.show_about = true;
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Robot, tr("Assistant IA…"))).clicked() {
                                self.assistant_settings = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.add(iconset::button(ctx, Ico::Profile, tr("Importer des profils…"))).clicked() {
                                self.import();
                                ui.close_menu();
                            }
                            if ui.add(iconset::button(ctx, Ico::Save, tr("Exporter les profils…"))).clicked() {
                                self.export();
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui
                                .button(if self.wanted {
                                    tr("Déconnecter le périphérique")
                                } else {
                                    tr("Connecter le périphérique")
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
                        if ui
                            .add(iconset::secondary_button(ctx, Ico::Robot, self.ai_status_label()))
                            .on_hover_text(tr("Assistant IA : IA locale sur ce PC et IA connectée (Claude, ChatGPT…)"))
                            .clicked()
                        {
                            self.assistant_settings = true;
                        }
                        if ui
                            .add(iconset::secondary_button(ctx, if self.preferences.dark { Ico::Sun } else { Ico::Moon }, if self.preferences.dark {
                                tr("Thème clair")
                            } else {
                                tr("Thème sombre")
                            }))
                            .on_hover_text(tr("Changer le thème de toute l’interface"))
                            .clicked()
                        {
                            self.preferences.dark = !self.preferences.dark; self.preferences.scheme = theme::current_scheme(self.preferences.dark).into();
                            theme::apply(ctx, self.preferences.dark);
                            self.save_preferences();
                        }
                        // Visible seulement pendant une exécution (Ctrl + Alt + F12 reste toujours actif).
                        if self.engine.running.load(Ordering::SeqCst)
                            && ui
                            .add(
                                theme::danger_button(
                                    RichText::new(tr("Arrêter les macros en cours"))
                                        .color(theme::DANGER),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(egui::Stroke::new(1.0_f32, theme::DANGER)),
                            )
                            .on_hover_text(tr("Arrête les macros en cours et met les touches en pause (Ctrl + Alt + F12). Les applications déjà ouvertes restent ouvertes."))
                            .clicked()
                        {
                            self.engine.stop();
                            self.gate.enabled = false;
                            self.log(tr("Macros arrêtées. Touches en pause.").into());
                        }
                        if theme::toggle(ui, &mut self.gate.enabled).changed() {
                            if !self.gate.enabled {
                                self.engine.stop();
                            } else {
                                self.gate.inspect = false;
                            }
                        }
                        let (label, color) = match self.gate.decision() {
                            Decision::Execute => (tr("Touches actives"), GREEN),
                            Decision::Inspect => ("Diagnostic", accent()),
                            Decision::Blocked(_) => (
                                if !self.gate.enabled {
                                    tr("En pause")
                                } else if !self.gate.connected {
                                    if self.wanted {
                                        tr("En attente")
                                    } else {
                                        tr("Déconnecté")
                                    }
                                } else if self.gate.dirty {
                                    tr("Modifications…")
                                } else {
                                    tr("Synchronisation…")
                                },
                                p.muted,
                            ),
                        };
                        if ctx.screen_rect().width() >= 1200.0 { ui.label(RichText::new(label).size(12.0).strong().color(color)); }
                    });
                });
            });
    }
    fn save_preferences(&mut self) -> bool {
        if !self.preferences_valid {
            self.fail(tr("Le fichier preferences.json est illisible et reste préservé. Restaure sa sauvegarde avant d’enregistrer.").into());
            return false;
        }
        match self
            .preferences
            .save(&self.path.with_file_name("preferences.json"))
        {
            Ok(()) => {
                if let Some(desktop) = &self.desktop {
                    let _ = desktop.tx.send(crate::desktop::Command::Triggers(self.preferences.triggers.clone()));
                }
                true
            },
            Err(e) => {
                self.fail(trf("Enregistrement de Mes binds impossible : {error}", &[("error", &e)]));
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
                    && from < self.device_keys()
                    && target < self.device_keys()
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
                    self.fail(trf("Bind incompatible : {error}", &[("error", &e)]));
                    return;
                }
                self.config.keys_mut()[target] = key;
            }
            DragItem::Action(key) => {
                let current = &mut self.config.keys_mut()[target];
                if current.steps.len() >= 256 {
                    self.fail(tr("Une macro contient au maximum 256 étapes.").into());
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
            self.fail(trf("Affectation refusée : {error}", &[("error", &e)]));
            return;
        }
        self.edited();
        let audio_unset = self.config.keys()[target].steps.iter().any(|s| matches!(s, Step::AudioSwitch { first, second, .. } if first.is_empty() || second.is_empty()));

        self.log(trf("Touche {key} mise à jour.{hint} Ctrl+Z pour annuler.", &[("key", &format!("{:02}", target + 1)), ("hint", &if audio_unset { tr(" Choisis maintenant les deux périphériques audio dans l'inspecteur.") } else { "" })]));
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
                    self.fail(trf("Conversion annulée : sauvegarde de {name}.json impossible ({error}).", &[("name", &name), ("error", &e)]));
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
        self.log(trf("{keys} action(s) de touches et {binds} de Mes binds converties en blocs natifs. Sauvegardes : *.avant-migration-{stamp}.json (Ctrl+Z annule côté touches).", &[("keys", &keys), ("binds", &binds), ("stamp", &stamp)]));
    }
    fn undo_edit(&mut self) {
        if let Some(previous) = self.history.pop() { self.future.push(self.config.clone()); self.config = previous; self.replaying = true; self.edited(); self.log(tr("Modification annulée.").into()); }
    }
    fn redo_edit(&mut self) {
        if let Some(next) = self.future.pop() { self.history.push(self.config.clone()); self.config = next; self.replaying = true; self.edited(); self.log(tr("Modification rétablie.").into()); }
    }
    fn copy_key(&mut self) { self.clipboard = Some(self.config.keys()[self.selected].clone()); self.log(tr("Touche copiée.").into()); }
    fn paste_key(&mut self, target: usize) { if let Some(key) = self.clipboard.clone() { self.assign(DragItem::Bind(key), target); } }
    fn duplicate_key(&mut self, from: usize) {
        let Some(target) = ((from+1)..self.device_keys()).find(|&i| Self::empty_key(&self.config.keys()[i])) else { self.log(tr("Aucune touche vide après celle-ci sur cette page.").into()); return; };
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
                .add_filter(
                    tr("Profil ou sauvegarde Stream Deck"),
                    &[
                        "StreamDeckProfilesBackup",
                        "streamdeckprofilesbackup",
                        "streamDeckProfile",
                        "streamdeckprofile",
                        "streamDeckAction",
                        "streamdeckaction",
                        "json",
                    ],
                )
                .pick_file()?;
            let result = crate::elgato::load_all(&path);
            Some(Box::new(move |app| match result {
                Ok(reports) => app.import_reports = reports,
                Err(e) => app.fail(trf("Import Stream Deck impossible : {error}", &[("error", &e)])),
            }))
        });
    }
    fn apply_elgato(&mut self) {
        if self.import_reports.is_empty() {
            return;
        }
        let reports = std::mem::take(&mut self.import_reports);
        if self.config.profiles.len() + reports.len() > 64 {
            self.fail(trf(
                "64 profils maximum (actuels: {current}, importés: {imported}).",
                &[("current", &self.config.profiles.len()), ("imported", &reports.len())]
            ));
            self.import_reports = reports;
            return;
        }
        let mut total_converted = 0;
        let mut total_warnings = 0;
        let result = (|| -> anyhow::Result<()> {
            let mut next = self.config.clone();
            let base_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos();
            for (r_idx, mut report) in reports.into_iter().enumerate() {
                total_converted += report.converted;
                total_warnings += report.warnings.len();
                let original = report.profile.name.clone();
                let mut suffix = 2;
                while next
                    .profiles
                    .iter()
                    .any(|p| p.name.eq_ignore_ascii_case(&report.profile.name))
                {
                    report.profile.name = format!("{original} ({suffix})");
                    suffix += 1;
                }
                if !report.icons.is_empty() {
                    let dir = self.path.parent().unwrap().join("assets").join(format!(
                        "import-{base_time}-{r_idx}"
                    ));
                    std::fs::create_dir_all(&dir)?;
                    for (i, (page, key, bytes)) in report.icons.iter().enumerate() {
                        let path = dir.join(format!("icon-{i}.png"));
                        std::fs::write(&path, bytes)?;
                        if let Some(keys) = report.profile.keys_at_mut(page) {
                            if let Some(target_key) = keys.get_mut(*key) {
                                target_key.image = path.to_string_lossy().into_owned();
                            }
                        }
                    }
                }
                next.profiles.push(report.profile);
            }
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
            Ok(()) => self.log(trf("Import Stream Deck terminé : {total_converted} actions converties, {total_warnings} avertissements. Touches en pause.", &[("total_converted", &total_converted), ("total_warnings", &total_warnings)])),
            Err(e) => {
                self.fail(trf("Import non enregistré : {error}", &[("error", &e)]));
            }
        }
    }
    fn open_crop_editor(&mut self, profile: usize, page: String, index: usize, path: std::path::PathBuf, size: u32) {
        let assets = self.path.parent().unwrap().join("assets/mes-icones");
        match crate::ui_media_crop::CropSession::open(path, profile, page, index, size, assets) {
            Ok(session) => {
                self.crop_session = Some(session);
            }
            Err(e) => {
                self.fail(trf("Impossible d'ouvrir le média pour recadrage : {error}", &[("error", &e)]));
            }
        }
    }
    fn import_binds(&mut self) {
        self.gate.enabled = false;
        self.engine.stop();
        self.start_dialog(|| {
            let path = rfd::FileDialog::new().add_filter(tr("Pack NovyDeck"), &["json"]).pick_file()?;
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
                            anyhow::bail!(tr("Pack non enregistré"));
                        }
                        Ok(count)
                    })();
                    match result {
                        Ok(n) => app.log(trf("{n} binds importés. Les touches sont en pause.", &[("n", &n)])),
                        Err(e) => app.fail(trf("Import refusé : {error}", &[("error", &e)])),
                    }
                }
                Err(e) => app.fail(trf("Import refusé : {error}", &[("error", &e)])),
            }))
        });
    }
    fn export_binds(&mut self) {
        self.gate.enabled = false;
        self.engine.stop();
        let library = self.preferences.library.clone();
        self.start_dialog(move || {
            let path = rfd::FileDialog::new().set_file_name("mes-binds-novydeck.json")
                .add_filter(tr("Pack NovyDeck"), &["json"]).save_file()?;
            let result = serde_json::to_vec_pretty(&library)
                .map_err(anyhow::Error::from)
                .and_then(|b| std::fs::write(path, b).map_err(Into::into));
            Some(Box::new(move |app| match result {
                Ok(()) => app.log(tr("Pack exporté. Les touches sont en pause.").into()),
                Err(e) => app.fail(trf("Export impossible : {error}", &[("error", &e)])),
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
                    ui.label(RichText::new(tr("Bibliothèque d'actions")).size(17.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        theme::pill(ui, tr("40 binds"), accent());
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
                                    .hint_text(tr("Rechercher une action…")),
                            );
                        });
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    for (i, label) in [tr("Actions"), tr("Magasin"), tr("Mes binds"), tr("Plugins")]
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
                        iconset::paint(
                            ui.painter(),
                            r.center_top() + Vec2::new(0.0, 15.0),
                            20.0,
                            [Ico::Actions, Ico::Shop, Ico::Bookmark, Ico::Plugin][i],
                            [Family::Accent, Family::Plugin, Family::Accent, Family::Plugin][i],
                            if selected { 1.0 } else { 0.55 },
                            [21, 20, 15, 15][i],
                        );
                        ui.painter().text(
                            r.center_bottom() - Vec2::new(0.0, 6.0),
                            egui::Align2::CENTER_BOTTOM,
                            *label,
                            egui::FontId::proportional(11.0),
                            if selected { p.ink } else { p.muted },
                        );
                        if response.clicked() {
                            if i == 3 && self.catalog_tab != 3 {
                                self.plugin_inventory = None;
                            }
                            self.catalog_tab = i;
                            self.catalog_selection = None;
                            self.catalog_search.clear();
                        }
                    }
                });
                ui.add_space(6.0);
                motion::fade(ui, "library_tab", self.catalog_tab, 0.15, self.animations());
                let search = self.catalog_search.to_lowercase();
                if self.catalog_tab == 3 {
                    self.plugin_store(ui, &search);
                } else if self.catalog_tab == 0 {
                    ui.label(
                        RichText::new(
                            tr("Glisse une action sur une touche. Elle s’ajoute à la fin de sa macro."),
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
                                    let badge = egui::Rect::from_center_size(
                                        r.left_center() + Vec2::new(23.0, 0.0),
                                        Vec2::splat(32.0),
                                    );
                                    let (ico, kind) = match &item {
                                        DragItem::Action(key) => (iconset::for_steps(&key.steps), crate::icons::kind(key)),
                                        _ => (Ico::Folder, 15),
                                    };
                                    ui.painter()
                                        .rect_filled(badge, 8.0, iconset::family_color(ui.ctx(), ico).gamma_multiply(0.18));
                                    iconset::paint(ui.painter(), badge.center(), 22.0, ico, ico.family(), 1.0, kind);
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
                                        tr("Action · Glisser ou cliquer"),
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
                                        tr("Glisser sur une touche, ou cliquer pour ajouter à la touche sélectionnée"),
                                    );
                                });
                            }
                            ui.add_space(14.0);
                            ui.label(RichText::new(tr("Macro personnalisée")).strong());
                            ui.label(
                                RichText::new(tr("Combine plusieurs actions dans l’inspecteur du bas, puis enregistre le résultat dans Mes binds."))
                                    .small()
                                    .color(p.muted),
                            );
                        });
                } else {
                    if self.catalog_tab == 1 {
                        egui::ComboBox::from_id_salt("catalog_category")
                            .width(ui.available_width() - 8.0)
                            .selected_text(crate::catalog::display_category(&self.catalog_category))
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
                                        crate::catalog::display_category(category),
                                    );
                                }
                            });
                        ui.label(
                            RichText::new(tr("Gratuit · hors ligne · modifiable"))
                                .small()
                                .color(p.muted),
                        );
                    } else {
                        ui.horizontal(|ui| {
                            if ui.button(tr("Enregistrer la touche")).clicked() {
                                self.bind_name = self.config.keys()[self.selected].label.clone();
                                self.save_bind = true;
                            }
                            ui.menu_button("•••", |ui| {
                                if ui.add(iconset::button(ui.ctx(), Ico::Album, tr("Importer un pack…"))).clicked() {
                                    self.import_binds();
                                    ui.close_menu();
                                }
                                if ui.button(tr("Exporter Mes binds…")).clicked() {
                                    self.export_binds();
                                    ui.close_menu();
                                }
                            });
                        });
                    }
                    let items: Vec<(String, String, Key)> = if self.catalog_tab == 1 {
                        catalog::presets()
                            .into_iter()
                            .map(|v| (v.category.into(), catalog::display_description(v.description).into(), v.key))
                            .collect()
                    } else {
                        self.preferences
                            .library
                            .binds
                            .iter()
                            .map(|b| {
                                (
                                    "Personnel".into(),
                                    trf(
                                        "{name} · {steps} étape(s). Les images restent liées à leur fichier local.",
                                        &[("name", &b.name), ("steps", &b.key.steps.len())]
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
                                                egui::Stroke::new(1.5_f32, accent()),
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
                                        let ico = iconset::for_steps(&key.steps);
                                        let _ = color;
                                        ui.painter().rect_filled(
                                            badge,
                                            9.0,
                                            iconset::family_color(ui.ctx(), ico).gamma_multiply(0.16),
                                        );
                                        iconset::paint(
                                            ui.painter(),
                                            badge.center(),
                                            22.0,
                                            ico,
                                            ico.family(),
                                            1.0,
                                            crate::icons::kind(key),
                                        );
                                        let mut title =
                                            egui::text::LayoutJob::simple_singleline(
                                                if self.catalog_tab == 1 { catalog::display_name(&key.label).to_owned() } else { key.label.clone() },
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
                                            if selected == Some(i) { accent() } else { p.muted },
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
                                    tr("Tes binds enregistrés apparaîtront ici.")
                                } else {
                                    tr("Aucun résultat. Essaie un autre mot.")
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
                                ui.label(RichText::new(if self.catalog_tab == 1 { catalog::display_name(&key.label) } else { key.label.as_str() }).strong());
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
                                        RichText::new(trf("Utiliser sur la touche {key}", &[("key", &format!("{:02}", self.selected + 1))]))
                                        .color(Color32::WHITE),
                                    ))
                                    .clicked()
                                {
                                    self.assign(DragItem::Bind(key.clone()), self.selected);
                                }
                                if self.catalog_tab == 2
                                    && ui.small_button(tr("Supprimer de Mes binds")).clicked()
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
                                tr("Glisse un bind sur une touche, ou clique pour voir ses actions."),
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
            trf("Brouillon · {error} · Les autres touches restent disponibles.", &[("error", &e)])
        } else if self.gate.dirty {
            tr("Sauvegarde automatique…").into()
        } else if self.notice == "Tout est prêt." || self.notice == "All set." {
            tr("Tout est prêt.").into()
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
                    // Icône seulement quand il faut agir : l'état normal reste discret.
                    let state = if self.error { Some(Ico::Error) } else if validation.is_some() { Some(Ico::Warning) } else { None };
                    if let Some(image) = state.and_then(|ico| iconset::image(ctx, ico, 14.0)) {
                        ui.add(image);
                    }
                    ui.add_sized([ (ui.available_width() - 230.0).max(80.0), 20.0 ], egui::Label::new(RichText::new(&message).size(11.0).color(
                        if self.error || validation.is_some() {
                            Color32::from_rgb(233, 119, 109)
                        } else {
                            p.muted
                        },
                    )).truncate()).on_hover_text(&message);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(iconset::button(ctx, Ico::Log, if self.show_logs {
                                tr("Masquer le journal")
                            } else {
                                tr("Journal")
                            }).small())
                            .clicked()
                        {
                            self.show_logs = !self.show_logs;
                        }
                        ui.label(
                            RichText::new(tr("Arrêt : Ctrl + Alt + F12"))
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
                        ui.label(RichText::new(tr("Activité récente")).strong());
                        if ui.small_button(tr("Effacer")).clicked() {
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
        egui::TopBottomPanel::bottom("key_inspector").default_height(if ctx.screen_rect().height()<800.0 {(ctx.screen_rect().height()*0.31).clamp(140.0,260.0)}else{260.0}).height_range(120.0..=(ctx.screen_rect().height()*if ctx.screen_rect().height()<600.0 {0.31} else {0.5}).max(140.0)).resizable(true).frame(egui::Frame::new().fill(p.surface).inner_margin(16)).show(ctx,|ui| {
            let tab_id = egui::Id::new("inspector_tab");
            let mut tab = ui.ctx().data_mut(|d| *d.get_persisted_mut_or_default::<u8>(tab_id));
            let configured = { let k = &self.config.keys()[self.selected]; !k.steps.is_empty() || !k.long_steps.is_empty() || k.plugin.is_some() };
            let narrow = ui.available_width() < 1180.0;
            let two_rows = ui.available_width() < 900.0;
            // Icônes des onglets et de l'Atelier seulement s'il reste la place (sinon les boutons se chevauchent).
            let roomy = ui.available_width() >= 820.0;
            let actions = |app: &mut Self, ui: &mut egui::Ui, narrow: bool, buttons: &mut Vec<egui::Rect>| {
                if narrow {
                    let response = ui.add_enabled(!running, theme::primary(tr("Simuler"))).on_hover_text(tr("Déroule les actions dans le journal, sans aucun effet sur le PC")); buttons.push(response.rect); if response.clicked() { app.preview(true); }
                    let response = ui.add_enabled(!running, egui::Button::new(tr("Tester pour de vrai"))).on_hover_text(tr("Exécute réellement les actions dans 3 s, sur la fenêtre active")); buttons.push(response.rect); if response.clicked() { app.preview(false); }
                    let response = if roomy { ui.add(iconset::button(ui.ctx(), Ico::Workshop, tr("Atelier de blocs"))) } else { ui.button(tr("Atelier de blocs")) }; buttons.push(response.rect); if response.clicked() { app.show_workflow = true; }
                    ui.menu_button("•••", |ui| {
                        if ui.button(tr("Enregistrer dans Mes binds")).clicked() { app.bind_name=app.config.keys()[app.selected].label.clone();app.save_bind=true;ui.close_menu(); }
                        if ui.add_enabled(!app.config.keys()[app.selected].steps.is_empty(), theme::danger_button(tr("Vider"))).clicked() { app.assign(DragItem::Bind(Key::default()),app.selected);ui.close_menu(); }
                    });
                } else {
                    if ui.add(theme::primary(RichText::new(tr("Atelier de blocs")).color(Color32::WHITE))).on_hover_text(tr("Composer l'automatisation avec des blocs visuels")).clicked(){app.show_workflow=true;}
                    if ui.button(tr("Enregistrer dans Mes binds")).clicked(){app.bind_name=app.config.keys()[app.selected].label.clone();app.save_bind=true;}
                    if ui.add_enabled(!app.config.keys()[app.selected].steps.is_empty(),theme::danger_button(tr("Vider"))).clicked(){app.assign(DragItem::Bind(Key::default()),app.selected);}
                    if ui.add_enabled(!running,egui::Button::new(tr("Tester pour de vrai (3 s)"))).on_hover_text(tr("Exécute réellement les actions dans 3 s, sur la fenêtre active")).clicked(){app.preview(false);}
                    if ui.add_enabled(!running,theme::primary(tr("Simuler"))).on_hover_text(tr("Déroule les actions dans le journal, sans aucun effet sur le PC")).clicked(){app.preview(true);}
                }
            };
            let mut buttons = Vec::new();
            let mut below = false;
            ui.horizontal(|ui| {
                if !two_rows { ui.label(RichText::new(trf("Touche {key}", &[("key", &format!("{:02}", self.selected+1))])).size(17.0).strong()); }
                if !narrow { theme::status_badge(ui, if configured { tr("Action configurée") } else { tr("Touche vide") }, if configured { GREEN } else { p.muted }); ui.add_space(10.0); }
                egui::Frame::new().fill(p.bg).corner_radius(10).inner_margin(egui::Margin::same(3)).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (i, ico, label) in [(0u8, Ico::Actions, tr("Action")), (1u8, Ico::Palette, tr("Apparence")), (2u8, Ico::Advanced, tr("Avancé"))] {
                        let response = if roomy { iconset::tab(ui, tab == i, ico, label) } else { iconset::plain_tab(ui, tab == i, label) };
                        if response.on_hover_text(label).clicked() { tab = i; }
                    }
                });
                if tab == 0 && two_rows {
                    let text = if self.long_editor { tr("Appui long") } else { tr("Appui court") };
                    let toggle = ui.button(text).on_hover_text(tr("Basculer entre appui court et appui long"));
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("inspector_press_toggle"), toggle.rect));
                    if toggle.clicked() { self.long_editor = !self.long_editor; }
                } else if tab == 0 {
                    ui.add_space(6.0);
                    ui.selectable_value(&mut self.long_editor, false, if narrow { tr("Court") } else { tr("Appui court") });
                    ui.selectable_value(&mut self.long_editor, true, if narrow { tr("Long") } else { tr("Appui long") });
                }
                // Pas assez de place à droite des onglets : les boutons passent sur une seconde ligne
                // plutôt que de recouvrir « Appui court ».
                let need: f32 = [tr("Simuler"), tr("Tester pour de vrai"), tr("Atelier de blocs"), "•••"].iter().map(|t| {
                    let font = egui::TextStyle::Button.resolve(ui.style());
                    ui.fonts(|f| f.layout_no_wrap((*t).to_owned(), font, Color32::WHITE).size().x)
                        + 2.0 * ui.spacing().button_padding.x + ui.spacing().item_spacing.x
                }).sum::<f32>() + if roomy { 22.0 } else { 0.0 };
                below = narrow && ui.available_width() < need;
                if !below {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| actions(self, ui, narrow, &mut buttons));
                }
            });
            if below {
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 30.0), egui::Layout::right_to_left(egui::Align::Center), |ui| actions(self, ui, narrow, &mut buttons));
            }
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("inspector_button_rects"), buttons));
            ui.ctx().data_mut(|d| d.insert_persisted(tab_id, tab));
            ui.separator();
            motion::fade(ui, "inspector", (tab, self.selected, self.long_editor, self.config.page.clone()), 0.25, self.animations());
            if let Some(shortcut) = extras::discord_shortcut(&self.config.keys()[self.selected]) {
                ui.group(|ui| {
                    ui.label(tr("Discord : configure ce raccourci dans Discord → Paramètres → Raccourcis clavier"));
                    ui.horizontal(|ui| {
                        ui.label(shortcut);
                        if ui.button(tr("Copier")).clicked() { ui.ctx().copy_text(shortcut.to_owned()); }
                    });
                    ui.label(tr("État estimé : NovyDeck ne lit pas l'état réel du micro ni du casque dans Discord."));
                });
            }
            let preview_key = self.presented_key(self.selected);
            let preview_face = self.plugin_face(self.selected).cloned();
            let preview = self.key_previews.texture(ctx, self.selected, &preview_key, self.config.image_size, preview_face.as_ref());
            let long_editor = self.long_editor;
            let active=self.config.active; let names=self.config.profiles.iter().map(|p|p.name.clone()).collect::<Vec<_>>();
            let pages=self.config.profiles[active].pages.clone();let selected=self.selected;let page_id=self.config.page.clone();let dialog_tx=self.dialog_tx.clone();let image_size=self.config.image_size;let key=&mut self.config.keys_mut()[selected];
            let mut removed_step=false; let mut changed=false; let mut add_kind=None; let mut record_step=None; let mut open_icons=false; let mut recrop=false; let mut state_icon=None; let mut pick_audio=None; let mut open_workflow=false; let mut open_model=false; let mut advanced: Option<u8>=None;
            if let Some(binding)=key.plugin.clone(){
                // Boutons d'abord, à droite : l'identifiant long du plugin est tronqué à gauche.
                let status=self.plugin_errors.get(&binding.plugin).cloned();
                let short=|id:&str|id.rsplit('.').next().unwrap_or(id).to_owned();
                let row=ui.spacing().interact_size.y;
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(),row),egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    if ui.button(tr("Retirer le plugin")).clicked(){key.plugin=None;changed=true;}
                    if self.plugins.as_ref().is_some_and(|m|m.inspector_path(selected).is_some())
                        && ui.button(tr("Réglages du plugin")).clicked(){if let Some(m)=&self.plugins {m.open_inspector(selected);}}
                    let text=format!("Plugin {} · {} — {}",short(&binding.plugin),short(&binding.action),status.as_deref().unwrap_or("Actif"));
                    let color=if status.is_some(){theme::WARNING}else{ui.visuals().text_color()};
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center),|ui|{
                        ui.add(egui::Label::new(RichText::new(text).color(color)).truncate())
                            .on_hover_text(format!("{} · {}\n{}",binding.plugin,binding.action,status.as_deref().unwrap_or("Actif")));
                    });
                });
            }
            if self.flicker.as_ref().is_some_and(|session| session.active()) {self.diagnostic_inspector=Some(ui.max_rect());}
            let width=ui.available_width(); let body_height=ui.available_height().max(40.0);
            // Une seule réservation : l'UI enfant dessine dedans sans allouer une deuxième hauteur.
            let (body, _) = ui.allocate_exact_size(Vec2::new(width, body_height), egui::Sense::hover());
            let mut body_ui = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::left_to_right(egui::Align::Min)));
            body_ui.horizontal_top(|ui| {
                let lower = key.image.to_ascii_lowercase();
                let can_recrop = !key.image.is_empty() && !lower.ends_with(".svg") && std::path::Path::new(&key.image).is_file();
                // Hauteur réservée sous l'aperçu : nom + « Changer l'icône », et « Recadrer » quand il est proposé.
                let button_height = ui.text_style_height(&egui::TextStyle::Button) + 2.0 * ui.spacing().button_padding.y;
                let controls = 62.0 + if can_recrop { button_height + ui.spacing().item_spacing.y + 6.0 } else { 0.0 };
                let roomy = body_height >= 150.0;
                let preview_width = (if width < 650.0 { 76.0 } else { 140.0_f32 }).min(if roomy { body_height - controls } else { body_height - 6.0 }).max(40.0);
                // La colonne garde la largeur des boutons même quand l'aperçu rétrécit : pas de libellé sur deux lignes.
                let button_width = if roomy { ui.fonts(|f| f.layout_no_wrap(tr("Changer l'icône").to_owned(), egui::TextStyle::Button.resolve(ui.style()), Color32::WHITE).size().x) + 2.0 * ui.spacing().button_padding.x } else { 0.0 };
                let preview_width_column = preview_width.max(button_width);
                ui.allocate_ui_with_layout(Vec2::new(preview_width_column + 18.0, body_height), egui::Layout::top_down(egui::Align::Center), |ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(preview_width), egui::Sense::hover());
                    theme::key_face(ui.painter(), rect, true, false);
                    let lcd = rect.shrink(5.0);
                    match &preview {
                        Ok(texture) => theme::rounded_image(ui.painter(), texture.id(), lcd, theme::KEY_LCD_RADIUS),
                        Err(_) => { ui.painter().text(lcd.center(), egui::Align2::CENTER_CENTER, tr("Aperçu indisponible"), egui::FontId::proportional(9.0), p.muted); },
                    }
                    if roomy {
                        ui.label(RichText::new(&key.label).strong());
                        if ui.button(tr("Changer l'icône")).clicked() { open_icons = true; }
                        if can_recrop
                            && ui.button(tr("Recadrer")).on_hover_text(tr("Ajuster le cadrage de l'image avec aperçu avant validation")).clicked() { recrop = true; }
                    }
                });
                ui.separator();
                ui.allocate_ui_with_layout(Vec2::new((width-preview_width_column-32.0).max(100.0),body_height),egui::Layout::top_down(egui::Align::Min),|ui| {
                    if tab == 2 {
                        egui::ScrollArea::vertical().id_salt("inspector_advanced").max_height(body_height).show(ui,|ui|{
                            ui.label(RichText::new(tr("Comportement")).strong());
                            ui.horizontal(|ui|{ui.label(tr("Répéter la macro"));changed|=ui.add(egui::DragValue::new(&mut key.repeat).range(1..=1000)).changed();ui.label(tr("fois à chaque appui"));});
                             ui.horizontal(|ui| {
                                 ui.label(tr("Si une macro est en cours"));
                                 egui::ComboBox::from_id_salt("key_run_mode").selected_text(match key.run_mode.unwrap_or_default() {
                                     crate::model::RunMode::Ignore => "Ignorer", crate::model::RunMode::Restart => tr("Redémarrer"), crate::model::RunMode::Queue => tr("Mettre en attente"),
                                 }).show_ui(ui, |ui| {
                                     changed |= ui.selectable_value(&mut key.run_mode, None, tr("Ignorer (défaut)")).changed();
                                     changed |= ui.selectable_value(&mut key.run_mode, Some(crate::model::RunMode::Restart), tr("Redémarrer")).changed();
                                     changed |= ui.selectable_value(&mut key.run_mode, Some(crate::model::RunMode::Queue), tr("Mettre en attente (5 max)")).changed();
                                 });
                             });
                             ui.horizontal(|ui| { ui.label(tr("Anti-rebond")); changed |= ui.add(egui::DragValue::new(&mut key.debounce_ms).range(0..=60_000)).changed(); ui.label(tr("ms par touche")); });
                            ui.label(RichText::new(trf("Appui court : {short} étape(s) · Appui long : {long} étape(s)", &[("short", &key.steps.len()), ("long", &key.long_steps.len())])).color(p.muted));
                            ui.add_space(8.0);
                            ui.label(RichText::new(tr("Touche")).strong());
                            ui.horizontal(|ui|{
                                if ui.button(tr("Copier la touche")).clicked(){advanced=Some(0);}
                                if ui.button(tr("Coller ici")).clicked(){advanced=Some(1);}
                                if ui.button(tr("Dupliquer vers la touche libre suivante")).clicked(){advanced=Some(2);}
                            });
                            ui.add_space(8.0);
                            ui.label(RichText::new(tr("La simulation ne lance aucune action. Le test réel vise la fenêtre active ; « Arrêter les macros en cours » (Ctrl + Alt + F12) interrompt une exécution.")).size(11.0).color(p.muted));
                        });
                        return;
                    }
                    if tab == 1 {
                        egui::ScrollArea::vertical().id_salt("key_appearance").auto_shrink([false, false]).max_height(body_height).show(ui,|ui| {
                            if key.steps.len()==1 && matches!(key.steps[0],Step::AudioSwitch{..}) {
                                let mut state=None;
                                changed |= icon_ui::audio_options(ui,&mut key.steps[0],&self.audio,&mut state,&dialog_tx,&(active,page_id.clone(),selected,vec![0]));
                                if let Some(state)=state { pick_audio=Some((vec![0],state)); }
                            }
                        ui.label(RichText::new(tr("Titre sur la touche")).small().color(p.muted));
                        changed|=ui.add(egui::TextEdit::singleline(&mut key.label).desired_width(200.0).hint_text(tr("Nom de la touche"))).changed();
                        ui.horizontal(|ui| {
                            ui.label(tr("Taille du titre"));
                            changed |= ui.add(egui::Slider::new(&mut key.title_size, 10..=20).show_value(false)).changed();
                            ui.label(format!("{} px", key.title_size));
                        });
                        changed|=appearance::color(ui,key);
                        ui.add_space(10.0);
                        ui.separator();
                        ui.label(RichText::new(tr("Affichage selon l'état")).strong());
                        let mut enabled = key.state_display.is_some();
                        if ui.checkbox(&mut enabled, tr("Adapter la touche à une condition réelle")).changed() {
                            key.state_display = enabled.then(crate::model::StateDisplay::default);
                            changed = true;
                        }
                        if let Some(display) = &mut key.state_display {
                            ui.push_id("state_display", |ui| {
                                changed |= edit_condition(ui, &mut display.condition, &self.audio, 0);
                                for (active, title, face) in [(true, tr("Quand c'est vrai"), &mut display.on), (false, tr("Sinon"), &mut display.off)] {
                                    theme::card_frame(theme::palette(ui.ctx())).show(ui, |ui| {
                                        ui.label(RichText::new(title).strong());
                                        let label = face.label.get_or_insert_with(String::new);
                                        changed |= ui.add(egui::TextEdit::singleline(label).hint_text(tr("Titre (vide = aucun titre)"))).changed();
                                        ui.horizontal(|ui| {
                                            let mut color = face.color.unwrap_or(key.color);
                                            if ui.color_edit_button_srgb(&mut color).changed() { face.color = Some(color); changed = true; }
                                            if ui.small_button(tr("Couleur par défaut")).clicked() { face.color = None; changed = true; }
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label(face.image.as_deref().filter(|v| !v.is_empty()).unwrap_or(tr("Icône de la touche")));
                                            if ui.button(tr("Galerie")).clicked() { state_icon = Some(active); }
                                            if ui.small_button(tr("Effacer")).clicked() { face.image = None; changed = true; }
                                        });
                                    });
                                }
                                ui.collapsing(tr("Options avancées"), |ui| {
                                    changed |= ui.add(egui::DragValue::new(&mut display.poll_ms).range(250..=600_000).suffix(" ms")).changed();
                                });
                            });
                        }
                        if crate::audio::single_switch(&key.steps).is_some(){if ui.button(tr("Icônes par état →")).on_hover_text(tr("Choisir séparément les icônes des deux états audio")).clicked(){open_icons=true;}}else{
                        let actions=appearance::icons(ui,!key.image.is_empty());
                        open_icons|=actions.gallery;
                        if actions.pick_file {
                            let profile=active;let page=page_id.clone();let index=selected;
                            let tx=self.dialog_tx.clone();
                            let assets=self.path.parent().unwrap().join("assets/mes-icones");
                            let size=image_size;
                            std::thread::spawn(move || {
                                let path=rfd::FileDialog::new().add_filter(tr("Image ou vidéo"), &["png","jpg","jpeg","svg","gif","mp4","webm","mov","mkv","avi"]).pick_file();
                                if let Some(path) = path {
                                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
                                    if ext != "svg" {
                                        let _ = tx.send(Some(Box::new(move |app: &mut App| {
                                            app.open_crop_editor(profile, page, index, path, size);
                                        }) as DialogEffect));
                                        return;
                                    }
                                    let video = crate::assets::is_video(&path);
                                    let src = path.clone(); let assets_dir = assets.clone();
                                    if video { let _ = tx.send(Some(Box::new(|app: &mut App| app.log(tr("Conversion de la vidéo…").into())) as DialogEffect)); }
                                    let result = if video {
                                        crate::assets::import_video(&path, &assets, size).map_err(|e| e.to_string())
                                    } else if path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("gif")) {
                                        std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| {
                                            let oversized = image::image_dimensions(&path).map(|(w,h)| w > 512 || h > 512).unwrap_or(false);
                                            if oversized { let _ = tx.send(Some(Box::new(|app: &mut App| app.log(tr("GIF trop grand : réduction automatique à 256 px.").into())) as DialogEffect)); }
                                            crate::assets::import_image(&bytes, &assets, path.file_stem().and_then(|s| s.to_str()).unwrap_or("gif")).map_err(|e| e.to_string())
                                        })
                                    } else { Ok(path) };
                                    let _=tx.send(Some(Box::new(move |app: &mut App| match result {
                                        Ok(path) => {
                                            if let Some(key)=app.config.profiles.get_mut(profile).and_then(|p|p.keys_at_mut(&page)).and_then(|keys|keys.get_mut(index)) {
                                                key.image=path.to_string_lossy().into_owned();app.edited();
                                                if video { app.log(tr("Vidéo convertie en GIF animé.").into()); }
                                            }
                                        }
                                        Err(e) => {
                                            if video && e.contains(crate::assets::FFMPEG_MISSING) {
                                                app.ffmpeg_offer = Some(FfmpegOffer { video: src, assets: assets_dir, size, profile, page, index });
                                            }
                                            app.fail(trf("Import image ou vidéo impossible : {error}", &[("error", &e)]));
                                        }
                                    }) as DialogEffect));
                                }
                            });
                        }
                        if actions.clear {key.image.clear();changed=true;}

                        }
                        ui.horizontal(|ui| {
                        ui.label(tr("Taille de l’icône"));
                            if ui.small_button(tr("Défaut")).on_hover_text(tr("Remettre la taille de l’icône à 100 %")).clicked() && key.icon_scale != 100 {
                                key.icon_scale = 100; changed = true;
                            }
                        });
                        changed |= ui.add(egui::Slider::new(&mut key.icon_scale, 20..=200).suffix(" %")).changed();
                        });
                        return;
                    }
                    let steps = if long_editor { &mut key.long_steps } else { &mut key.steps };
                    if long_editor { ui.label(RichText::new(tr("L’atelier de blocs modifie uniquement l’appui court. Pour un fichier, renseigne son chemin dans l’action.")).small().color(p.muted)); }
                    ui.horizontal(|ui|{
                        ui.label(RichText::new(trf("Actions · {steps} étape(s)", &[("steps", &steps.len())])).strong());
                        iconset::menu(ui, Ico::Add, "Ajouter",|ui|{for (i,label) in crate::automation::NAMES.iter().enumerate(){if ui.add_enabled(steps.len()<256,iconset::button(ui.ctx(), iconset::for_step(&crate::automation::step(i)), *label)).clicked(){add_kind=Some(i);ui.close_menu();}}});
                    });
                    let mut remove=None;let mut movement=None;
                    egui::ScrollArea::vertical().id_salt("step_list").auto_shrink([false, false]).max_height((body_height-40.0).max(100.0)).show(ui,|ui| {
                        if steps.is_empty(){ui.add_space(12.0);ui.label(RichText::new(tr("Glisse une action ou un bind sur cette touche.")).strong());ui.label(RichText::new(tr("La bibliothèque à droite propose des raccourcis prêts à l’emploi. Tu peux ensuite combiner et modifier leurs actions ici.")).color(p.muted));}
                        if steps.is_empty(){ui.horizontal(|ui| {if ui.add(iconset::button(ui.ctx(), Ico::Workshop, tr("Ouvrir l’Atelier de blocs"))).clicked(){open_workflow=true;}if ui.add(iconset::button(ui.ctx(), Ico::Template, tr("Partir d’un modèle"))).clicked(){open_model=true;}});}
                        let count=steps.len();
                        for(i,step) in steps.iter_mut().enumerate(){ui.push_id(i,|ui|{
                            egui::Frame::new().fill(p.bg).corner_radius(6).inner_margin(10).show(ui,|ui| {
                                ui.set_max_width(ui.available_width());
                                ui.horizontal(|ui|{
                                    ui.label(RichText::new(format!("{:02}",i+1)).color(accent()).size(11.0));
                                    ui.label(RichText::new(step.name()).strong().size(12.0));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                                        if ui.small_button("×").on_hover_text(tr("Supprimer cette étape")).clicked(){remove=Some(i);}
                                        if ui.add_enabled(i+1<count,egui::Button::new("↓").small()).clicked(){movement=Some((i,i+1));}
                                        if ui.add_enabled(i>0,egui::Button::new("↑").small()).clicked(){movement=Some((i,i-1));}
                                    });
                                });
                                if matches!(step,Step::AutoHotkey{..}) {ui.label(RichText::new(tr("Pas besoin d’écrire du code : compose tes actions dans l’Atelier de blocs")).small().color(p.muted));if ui.button(tr("Ouvrir l’Atelier de blocs")).clicked(){open_workflow=true;}}
                                let mut record=false;let mut audio_icon=None;changed|=edit_step(ui,step,&names,&pages,&mut record,&self.audio,&mut audio_icon,&dialog_tx,&(active,page_id.clone(),selected,vec![i]));if let Some(state)=audio_icon{pick_audio=Some((vec![i],state));}if record && !long_editor {record_step=Some(i);}
                            });
                        });}
                    });
                    if let Some(i)=remove{steps.remove(i);changed=true;removed_step=true;}if let Some((a,b))=movement{steps.swap(a,b);changed=true;}
                });
            });
            match advanced { Some(0)=>self.copy_key(), Some(1)=>self.paste_key(self.selected), Some(2)=>self.duplicate_key(self.selected), _=>{} }
            if open_workflow {self.show_workflow=true;}if open_model {self.show_workflow=true;}
            if let Some(step)=record_step {self.recording=Some((active,self.config.page.clone(),self.selected,vec![step],self.gate.enabled));self.recording_error.clear();self.gate.enabled=false;self.engine.stop();self.refresh_hotkeys();}
            if changed {self.edited(); if removed_step {self.log(tr("Étape retirée — Ctrl+Z pour annuler").into());}}
            if open_icons{self.open_icon_gallery(None);}
            if recrop {
                let path = std::path::PathBuf::from(&self.config.keys()[selected].image);
                let page = self.config.page.clone();
                self.open_crop_editor(active, page, selected, path, self.config.image_size);
            }if let Some(target)=pick_audio{self.open_icon_gallery(Some(target));}
            if let Some(active)=state_icon { self.open_state_icon_gallery(active); }
            if let Some(i)=add_kind {if i==12{self.create_page(true);}else if long_editor {
                let action = catalog::action(i,&names[active]);
                self.config.keys_mut()[selected].long_steps.extend(action.steps);
                self.edited();
            }else{self.assign(DragItem::Action(catalog::action(i,&names[active])),self.selected);}}

        });
    }

    /// Une touche de la grille : face, aperçu LCD, repères et interactions.
    /// Commun au châssis SVG (5 × 3, `chassis` = échelle) et à l'ancien habillage (autres formats).
    #[allow(clippy::too_many_arguments)]
    fn deck_key(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, i: usize, r: egui::Rect, response: egui::Response, width: f32, chassis: Option<f32>) {
        let p = theme::palette(ctx);

                                            let mut key = self.presented_key(i);
                                            let face = self.plugin_face(i).cloned();
                                            if let Some(face) = &face {
                                                key = self.plugin_render_key(i, key, face);
                                            }
                                            let selected = i == self.selected;
                                            let flashing = self.last_key.is_some_and(|(idx, t)| {
                                                idx == i && t.elapsed() < Duration::from_millis(350)
                                            });
                                            let hovering = self.drag.moving()
                                                && self
                                                    .drag
                                                    .position
                                                    .is_some_and(|pos| r.contains(pos));
                                            match chassis {
                                                Some(s) => crate::deck_chassis::paint_face(ui.painter(), r, s, selected || hovering, flashing),
                                                None => theme::key_face(ui.painter(), r, selected || hovering, flashing),
                                            }
                                            let lcd = chassis.map_or(r.shrink(5.0), |s| crate::deck_chassis::lcd(r, s));
                                            let lcd_radius = chassis.map_or(theme::KEY_LCD_RADIUS, |s| crate::deck_chassis::lcd_radius(r, s));
                                            match self.key_previews.texture(
                                                ctx,
                                                i,
                                                &key,
                                                self.config.image_size,
                                                face.as_ref(),
                                            ) {
                                                Ok(texture) => theme::rounded_image(ui.painter(), texture.id(), lcd, lcd_radius),
                                                Err(error) => {
                                                    ui.painter().text(
                                                        lcd.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        tr("Aperçu indisponible"),
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
                                                    r.width().min(r.height()) * 0.22,
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
                                                if ui.button(tr("Copier")).clicked() {
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
                                                    .button(tr("Dupliquer vers la prochaine touche vide"))
                                                    .clicked()
                                                {
                                                    self.duplicate_key(i);
                                                    ui.close_menu();
                                                }
                                                if ui.button(tr("Vider")).clicked() {
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

                                            response.on_hover_text(trf("Touche {key} · {label}\nGlisse depuis la bibliothèque pour affecter un bind. Glisse une touche sur une autre pour les échanger.", &[("key", &format!("{:02}", i + 1)), ("label", &key.label)]));
    }
    fn deck(&mut self, ctx: &egui::Context) {
        let p = theme::palette(ctx);
        let grid = self.grid();
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
                        if ui.button(tr("← Retour")).clicked() {
                            self.navigate(Step::Back);
                        }
                        ui.label(RichText::new(tr("DOSSIER")).small().color(p.muted));
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
                            .on_hover_text(tr("Page précédente"))
                            .clicked()
                        {
                            self.navigate(Step::PagePrevious);
                        }
                        ui.label(
                            RichText::new(trf(
                                "Page {current} / {total}",
                                &[("current", &current_page_num), ("total", &total_pages)]
                            ))
                            .size(13.0)
                            .strong(),
                        );
                        if ui
                            .small_button("›")
                            .on_hover_text(tr("Page suivante"))
                            .clicked()
                        {
                            self.navigate(Step::PageNext);
                        }
                    }
                    if !self.config.page.is_empty() {
                        ui.menu_button(tr("Renommer"), |ui| {
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
                // Changement de page, de dossier ou de profil : les touches apparaissent en fondu.
                motion::fade(ui, "deck_page", (self.config.active, self.config.page.clone()), 0.2, self.animations());
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
                if grid.keys < self.config.keys().len() {
                    ui.label(RichText::new(trf("{keys} touches supplémentaires conservées, masquées sur ce périphérique", &[("keys", &(self.config.keys().len() - grid.keys))])).small().color(p.muted));
                }
                ui.add_space(if ctx.screen_rect().height() < 600.0 { 2.0 } else { 7.0 });
                if ctx.screen_rect().width() >= 1200.0 {
                    ui.label(
                        RichText::new(
                            tr("Glisse un bind sur une touche, puis personnalise ses actions dans l’inspecteur."),
                        )
                        .small()
                        .color(p.muted),
                    );
                }
                if crate::deck_chassis::applies(grid.cols, grid.rows) {
                    // Châssis SVG (couche 1), faces dynamiques (couche 2), aperçus LCD (couche 3).
                    // Bornée à l'écran : à la première image, la zone disponible peut être démesurée.
                    let available = ui.available_rect_before_wrap().intersect(ctx.screen_rect());
                    let chassis = crate::deck_chassis::fit(available);
                    ui.allocate_rect(egui::Rect::from_min_max(available.min, egui::pos2(available.max.x, chassis.max.y)), egui::Sense::hover());
                    let s = crate::deck_chassis::scale(chassis);
                    let width = crate::deck_chassis::FACE_SIZE.x * s;
                    if self.flicker.as_ref().is_some_and(|session| session.active()) {
                        self.diagnostic_key_width = width;
                    }
                    if !crate::deck_chassis::paint(ui.painter(), chassis) {
                        theme::device_shell(ui.painter(), chassis);
                    }
                    for i in 0..grid.keys {
                        let r = crate::deck_chassis::face(chassis, i);
                        let response = ui.interact(r, ui.id().with(("deck_key", i)), egui::Sense::click_and_drag());
                        self.deck_key(ctx, ui, i, r, response, width, Some(s));
                    }
                    ui.painter().text(
                        crate::deck_chassis::svg_point(chassis, egui::pos2(700.0, 783.0)),
                        egui::Align2::CENTER_CENTER,
                        "NOVYDECK",
                        egui::FontId::proportional((20.0 * s).clamp(7.0, 12.0)),
                        Color32::from_rgba_unmultiplied(150, 172, 204, 150),
                    );
                } else {
                    // Le pavé suit la taille de la fenêtre : il occupe tout l'espace disponible, sans barre de défilement.
                    let width = grid.key_width(ui.available_width(), ui.available_height());
                    if self.flicker.as_ref().is_some_and(|session| session.active()) {
                        self.diagnostic_key_width = width;
                    }
                    let total = grid.total_width(width);
                    ui.horizontal(|ui| {
                        ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                        let body = egui::Rect::from_min_size(
                            ui.cursor().min,
                            Vec2::new(total, grid.total_height(width)),
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
                                        for i in 0..grid.keys {
                                            let (r, response) = ui.allocate_exact_size(
                                                Vec2::splat(width),
                                                egui::Sense::click_and_drag(),
                                            );
                                            self.deck_key(ctx, ui, i, r, response, width, None);
                                            if i % grid.cols == grid.cols - 1 {
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
                            "D E C K F O R G E",
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
            egui::Window::new(tr("Enregistrer un raccourci")).open(&mut open).collapsible(false).resizable(false).default_width(410.0).show(ctx,|ui|{
                ui.label(RichText::new(tr("Appuie sur la combinaison souhaitée")).size(20.0).strong());
                ui.label(tr("Exemple : maintiens Ctrl + Maj, puis appuie sur S."));
                ui.label(RichText::new(tr("Seules les touches reçues par cette fenêtre sont capturées. Aucun enregistrement en arrière-plan. Les raccourcis réservés à Windows peuvent quitter la fenêtre.")).small().color(p.muted));
                if !self.recording_error.is_empty(){ui.colored_label(Color32::LIGHT_RED,&self.recording_error);}
                if ui.button(tr("Annuler l’enregistrement")).clicked(){cancel=true;}
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
            egui::Window::new(tr("Pages et dossiers"))
                .open(&mut open)
                .default_width(480.0)
                .show(ctx, |ui| {
                    ui.label(tr("Renomme ou ouvre les pages et dossiers de ce profil."));
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
                                        iconset::paint(
                                            ui.painter(),
                                            r.center(),
                                            18.0,
                                            if page.folder { Ico::Folder } else { Ico::Page },
                                            Family::Accent,
                                            1.0,
                                            if page.folder { 15 } else { 17 },
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
                                        if ui.small_button(tr("Supprimer…")).clicked() {
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
            egui::Window::new(tr("Supprimer cette page ?")).open(&mut open).collapsible(false).resizable(false).default_width(380.0).show(ctx,|ui|{
                ui.label(tr("Son contenu et les touches qui ouvrent cette page seront retirés du profil. Les autres pages seront conservées."));
                if ui.add(iconset::button(ui.ctx(), Ico::Delete, RichText::new(tr("Supprimer la page")).color(Color32::LIGHT_RED))).clicked(){remove=true;}
            });
            if remove {
                self.remove_page(&id);
                self.delete_page = None;
            } else if !open {
                self.delete_page = None;
            }
        }
        if !self.import_reports.is_empty() {
            let mut open = true;
            let mut install = false;
            egui::Window::new(if self.import_reports.len() > 1 {
                tr("Import Sauvegarde Stream Deck — aperçu")
            } else {
                tr("Import Stream Deck — aperçu")
            })
            .open(&mut open)
            .collapsible(false)
            .default_width(580.0)
            .show(ctx, |ui| {
                if self.import_reports.len() == 1 {
                    let report = &self.import_reports[0];
                    ui.label(RichText::new(&report.profile.name).size(22.0).strong());
                    ui.label(trf(
                        "{actions} actions converties · {pages} pages/dossiers · {icons} icônes",
                        &[("actions", &report.converted), ("pages", &(report.profile.pages.len() + 1)), ("icons", &report.icons.len())]
                    ));
                } else {
                    let total_actions: usize = self.import_reports.iter().map(|r| r.converted).sum();
                    let total_icons: usize = self.import_reports.iter().map(|r| r.icons.len()).sum();
                    ui.label(
                        RichText::new(trf(
                            "Sauvegarde complète : {profiles} profils détectés",
                            &[("profiles", &self.import_reports.len())]
                        ))
                        .size(20.0)
                        .strong(),
                    );
                    ui.label(trf("{total_actions} actions converties au total · {total_icons} icônes", &[("total_actions", &total_actions), ("total_icons", &total_icons)]));
                }
                ui.label(
                    RichText::new(
                        tr("Les nouveaux profils seront ajoutés à NovyDeck. Les actions non reconnues restent vides. Les touches restent en pause après l’import."),
                    )
                    .small()
                    .color(p.muted),
                );
                egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                    for report in &self.import_reports {
                        if self.import_reports.len() > 1 {
                            ui.add_space(4.0);
                            ui.heading(&report.profile.name);
                        }
                        for warning in &report.warnings {
                            ui.colored_label(Color32::from_rgb(222, 165, 77), warning);
                        }
                        for key in report.profile.keys.iter().filter(|k| !k.label.is_empty()) {
                            ui.label(format!("{} · {} action(s)", key.label, key.steps.len()));
                        }
                    }
                });
                let btn_text = if self.import_reports.len() > 1 {
                    trf("Importer tous les profils ({profiles})", &[("profiles", &self.import_reports.len())])
                } else {
                    tr("Importer ce profil").to_string()
                };
                if ui.add(theme::primary(RichText::new(btn_text).color(Color32::WHITE))).clicked() {
                    install = true;
                }
            });
            if install {
                self.apply_elgato();
            } else if !open {
                self.import_reports.clear();
            }
        }
        if let Some(mut session) = self.crop_session.take() {
            let mut open = true;
            let result = session.ui(ctx, &mut open);
            if let Some(res) = result {
                match res {
                    Ok(saved_path) => {
                        let profile = session.profile;
                        let page = session.page.clone();
                        let index = session.index;
                        let is_vid = session.is_video;
                        if let Some(key) = self.config.profiles.get_mut(profile).and_then(|p| p.keys_at_mut(&page)).and_then(|keys| keys.get_mut(index)) {
                            key.image = saved_path.to_string_lossy().into_owned();
                            self.edited();
                            if is_vid {
                                self.log(tr("Vidéo recadrée et convertie en GIF animé pour la touche D6.").into());
                            } else {
                                self.log(tr("Image recadrée et appliquée à la touche D6.").into());
                            }
                        }
                    }
                    Err(e) => {
                        self.fail(trf("Erreur de recadrage : {error}", &[("error", &e)]));
                    }
                }
            } else if open {
                self.crop_session = Some(session);
            }
        }
        if self.save_bind {
            let mut open = true;
            let mut save = false;
            egui::Window::new(tr("Enregistrer dans Mes binds"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(380.0)
                .show(ctx, |ui| {
                    ui.label(tr("Conserve une copie de cette touche et de toutes ses actions."));
                    ui.label(
                        RichText::new(tr("Tu pourras ensuite la glisser sur n’importe quelle touche."))
                            .small()
                            .color(p.muted),
                    );
                    ui.text_edit_singleline(&mut self.bind_name);
                    let key = &self.config.keys()[self.selected];
                    ui.label(trf(
                        "{actions} action(s) · répétée(s) {repeat} fois",
                        &[("actions", &key.steps.len()), ("repeat", &key.repeat)]
                    ));
                    if ui
                        .add_enabled(
                            !key.steps.is_empty() && !self.bind_name.trim().is_empty(),
                            theme::primary(
                                RichText::new(tr("Enregistrer le bind")).color(Color32::WHITE),
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
                            self.log(tr("Bind enregistré dans Mes binds.").into());
                        } else {
                            self.preferences.library = previous;
                        }
                    }
                    Err(e) => self.fail(trf("Bind non enregistré : {error}", &[("error", &e)])),
                }
            }
            self.save_bind = open;
        }
        if let Some(rx) = &self.update_rx {
            let mut finished = false;
            while let Ok(event) = rx.try_recv() {
                match event {
                    about::UpdateEvent::Checked(result) => {
                        self.update_result = Some(result);
                        self.update_phase = None;
                        finished = true;
                    }
                    about::UpdateEvent::Verifying => self.update_phase = Some(tr("Vérification…")),
                    about::UpdateEvent::Downloaded(result) => {
                        self.update_phase = None;
                        finished = true;
                        match result {
                            Err(error) => self.update_result = Some(Err(trf("Mise à jour impossible : {error}", &[("error", &error)]))),
                            Ok(setup) => {
                                // Failure to persist must never silently discard edits on restart.
                                if self.gate.invalid || self.config.validate_structure().is_err() {
                                    self.update_result = Some(Err(tr("Configuration invalide : enregistrez-la avant de mettre à jour").into()));
                                } else if let Err(error) = store::save(&self.path, &self.config) {
                                    self.update_result = Some(Err(trf("Sauvegarde impossible : {error}", &[("error", &error)])));
                                } else {
                                    self.gate.dirty = false;
                                    match crate::app_update::launch(&setup) {
                                        Ok(()) => {
                                            self.quitting = true; // évite la minimisation dans la barre système
                                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                        }
                                        Err(error) => self.update_result = Some(Err(trf("Démarrage de l'installateur impossible : {error}", &[("error", &error)]))),
                                    }
                                }
                                if self.update_result.as_ref().is_some_and(Result::is_err) {
                                    let _ = std::fs::remove_file(&setup);
                                    if let Some(parent) = setup.parent() { let _ = std::fs::remove_dir(parent); }
                                }
                            }
                        }
                    }
                }
            }
            if finished { self.update_rx = None; }
        }
        if self.startup_prompts && !self.whats_new && self.preferences.update_check == Some(true) && !self.startup_checked {
            self.start_update_check(ctx, true);
        }
        let startup_result = self.startup_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(result) = startup_result {
            self.startup_rx = None;
            match result {
                Ok(crate::app_update::Status::New(latest)) => self.startup_offer = Some(latest),
                Err(error) => self.log(trf("Vérification au démarrage impossible : {error}", &[("error", &error)])),
                _ => {}
            }
        }
        if self.startup_prompts && self.whats_new {
            let mut open = true;
            let mut understood = false;
            egui::Window::new(trf("Quoi de neuf dans NovyDeck {version}", &[("version", &env!("CARGO_PKG_VERSION"))]))
                .open(&mut open).collapsible(false).resizable(true).default_width(450.0)
                .show(ctx, |ui| {
                    about::notes(ui, &self.whats_new_notes);
                    if ui.button(tr("Compris")).clicked() { understood = true; }
                });
            if understood || !open {
                self.whats_new = false;
                self.preferences.last_seen_version = env!("CARGO_PKG_VERSION").into();
                self.save_preferences();
            }
        } else if self.startup_prompts && self.preferences.update_check.is_none() {
            let mut open = true;
            let mut choice = None;
            egui::Window::new(tr("Mises à jour automatiques"))
                .open(&mut open).collapsible(false).resizable(false).default_width(420.0)
                .show(ctx, |ui| {
                    ui.label(tr("NovyDeck peut vérifier au démarrage si une nouvelle version est disponible. Il contacte seulement GitHub pour lire le numéro de la dernière version ; rien n'est installé sans votre accord."));
                    ui.horizontal(|ui| {
                        if ui.button(tr("Vérifier au démarrage")).clicked() { choice = Some(true); }
                        if ui.button(tr("Non merci")).clicked() { choice = Some(false); }
                    });
                });
            if let Some(enabled) = choice {
                self.preferences.update_check = Some(enabled);
                self.save_preferences();
                if enabled { self.start_update_check(ctx, true); }
            } else if !open {
                self.preferences.update_check = Some(false);
                self.save_preferences();
            }
        }
        if let Some(latest) = self.startup_offer.clone() {
            let mut open = true;
            let mut later = false;
            egui::Window::new(tr("Mise à jour disponible"))
                .open(&mut open).collapsible(false).resizable(true).default_width(450.0)
                .show(ctx, |ui| {
                    ui.label(trf("NovyDeck {version} est disponible (vous avez {current}).", &[("version", &latest.version), ("current", &env!("CARGO_PKG_VERSION"))]));
                    about::notes(ui, &latest.notes);
                    if self.update_installed {
                        if ui.button(tr("Télécharger et installer")).clicked() {
                            self.update_confirm = Some(latest.clone());
                            later = true;
                        }
                    } else {
                        ui.label(tr("Version portable : téléchargez le ZIP depuis la page des nouveautés"));
                    }
                    if ui.button(tr("Voir les nouveautés")).clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(latest.page.clone()));
                    }
                    if ui.button(tr("Plus tard")).clicked() { later = true; }
                });
            if later || !open { self.startup_offer = None; }
        }
        if self.show_about {
            let mut open = true;
            egui::Window::new(tr("À propos de NovyDeck"))
                .open(&mut open).collapsible(false).resizable(false).default_width(370.0)
                .show(ctx, |ui| {
                    ui.label(RichText::new("NovyDeck").size(20.0).strong());
                    let legacy_audio = {
                        let mut config = self.config.clone();
                        let mut binds = self.preferences.library.binds.clone();
                        config.migrate_focus_audio() + binds.iter_mut().map(|b| crate::model::migrate_focus_audio_key(&mut b.key)).sum::<usize>()
                    };
                    if legacy_audio > 0 {
                        ui.colored_label(egui::Color32::from_rgb(240, 180, 60), trf("{legacy_audio} action(s) utilisent l’ancien module FocusAudio.exe.", &[("legacy_audio", &legacy_audio)]));
                        if ui.button(tr("Convertir en blocs natifs « Volume d’une application »")).on_hover_text(tr("Sauvegarde config.json et preferences.json à côté avant la conversion. Titres, icônes et couleurs restent inchangés.")).clicked() {
                            self.migrate_focus_audio_now();
                        }
                        ui.separator();
                    }
                    ui.label(format!("Version : {}", env!("CARGO_PKG_VERSION")));
                    ui.label(format!("Compilation : {} ({})", about::HASH, about::DATE));
                    if ui.button(tr("Copier ces informations")).clicked() {
                        ui.ctx().copy_text(about::information());
                    }
                    if ui.button(tr("Ouvrir la page GitHub")).clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab("https://github.com/HatiFlash/NovyDeck"));
                    }
                    about::hardware_report_button(ui, self);
                    ui.separator();
                    ui.label(tr("Icônes d'interface : IconPark (ByteDance), licence Apache 2.0."));
                    egui::CollapsingHeader::new(tr("Licence IconPark (Apache 2.0)")).show(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                            ui.label(RichText::new(iconset::LICENSE).monospace().size(10.0));
                        });
                    });
                    ui.separator();
                    if ui.add_enabled(self.update_rx.is_none() && self.update_confirm.is_none(), egui::Button::new(tr("Vérifier les mises à jour"))).clicked() {
                        self.start_update_check(ctx, false);
                    }
                    if let Some(phase) = self.update_phase { motion::loading(ui, phase); }
                    if let Some(result) = &self.update_result {
                        match result {
                            Err(error) => { ui.label(error); }
                            Ok(crate::app_update::Status::Current) => { ui.label(trf("À jour (version {version})", &[("version", &env!("CARGO_PKG_VERSION"))])); }
                            Ok(crate::app_update::Status::Ahead) => { ui.label(tr("Cette version est plus récente que la dernière publiée")); }
                            Ok(crate::app_update::Status::New(latest)) => {
                                ui.label(trf("Nouvelle version {version} disponible", &[("version", &latest.version)]));
                                about::notes(ui, &latest.notes);
                                if self.update_installed {
                                    if ui.add_enabled(self.update_rx.is_none(), egui::Button::new(tr("Télécharger et installer"))).clicked() {
                                        self.update_confirm = Some(latest.clone());
                                    }
                                } else {
                                    ui.label(tr("Version portable : téléchargez le ZIP depuis la page des nouveautés"));
                                }
                                if ui.button(tr("Voir les nouveautés")).clicked() {
                                    ui.ctx().open_url(egui::OpenUrl::new_tab(latest.page.clone()));
                                }
                            }
                        }
                    }
                });
            self.show_about = open;
        }
        if let Some(latest) = self.update_confirm.clone() {
            let mut confirm = true;
            egui::Window::new(tr("Installer la mise à jour"))
                .open(&mut confirm).collapsible(false).resizable(false).default_width(390.0)
                .show(ctx, |ui| {
                    ui.label(trf("Nouvelle version {version} disponible", &[("version", &latest.version)]));
                    about::notes(ui, &latest.notes);
                    ui.label(trf("NovyDeck va se fermer, installer la version {version} puis redémarrer. Vos profils, médias et plugins sont conservés.", &[("version", &latest.version)]));
                    ui.horizontal(|ui| {
                        if ui.button(tr("Installer maintenant")).clicked() {
                            self.update_confirm = None;
                            self.update_phase = Some(tr("Téléchargement…"));
                            let (tx, rx) = mpsc::channel();
                            self.update_rx = Some(rx);
                            let context = ctx.clone();
                            std::thread::spawn(move || {
                                let signal = tx.clone();
                                let result = crate::app_update::download_with_progress(&latest, || {
                                    let _ = signal.send(about::UpdateEvent::Verifying);
                                    context.request_repaint();
                                }).map_err(|e| e.to_string());
                                let _ = tx.send(about::UpdateEvent::Downloaded(result));
                                context.request_repaint();
                            });
                        }
                        if ui.button(tr("Annuler")).clicked() { self.update_confirm = None; }
                    });
                });
            if !confirm { self.update_confirm = None; }
        }
        if let Some(offer) = self.ffmpeg_offer.clone() {
            let mut open = true;
            let mut install = false;
            let installing = self.ffmpeg_installing;
            egui::Window::new(tr("Importer une vidéo")).id(egui::Id::new("ffmpeg_offer"))
                .open(&mut open).collapsible(false).resizable(false).default_width(380.0)
                .show(ctx, |ui| {
                    ui.label(tr("Windows n’a pas réussi à convertir cette vidéo (format ou codec non pris en charge). NovyDeck peut télécharger son module vidéo FFmpeg, gratuit, une seule fois."));
                    ui.add_space(6.0);
                    if installing {
                        motion::loading(ui, tr("Téléchargement du module vidéo (≈ 110 Mo) puis conversion…"));
                    } else {
                        if ui.add(iconset::button(ui.ctx(), Ico::Video, tr("Télécharger le module vidéo et importer"))).clicked() { install = true; }
                        ui.label(RichText::new(tr("Source officielle gyan.dev, empreinte vérifiée, rangé dans les données utilisateur (NovyDeck, ou DeckForge existant)\\tools. Sans droits administrateur. Autre solution : copier ffmpeg.exe à côté de NovyDeck.exe.")).small());
                    }
                });
            if install {
                self.ffmpeg_installing = true;
                let tx = self.dialog_tx.clone();
                std::thread::spawn(move || {
                    let result = crate::assets::install_ffmpeg()
                        .and_then(|_| crate::assets::import_video(&offer.video, &offer.assets, offer.size))
                        .map_err(|e| e.to_string());
                    let _ = tx.send(Some(Box::new(move |app: &mut App| {
                        app.ffmpeg_installing = false;
                        app.ffmpeg_offer = None;
                        match result {
                            Ok(path) => {
                                if let Some(key) = app.config.profiles.get_mut(offer.profile).and_then(|p| p.keys_at_mut(&offer.page)).and_then(|keys| keys.get_mut(offer.index)) {
                                    key.image = path.to_string_lossy().into_owned();
                                    app.edited();
                                }
                                app.log(tr("Module vidéo installé, vidéo convertie en GIF animé.").into());
                            }
                            Err(e) => app.fail(trf("Import vidéo impossible : {error}", &[("error", &e)])),
                        }
                    }) as DialogEffect));
                });
            }
            if !open && !self.ffmpeg_installing { self.ffmpeg_offer = None; }
        }
        if self.show_apps {
            let mut open = true;
            egui::Window::new(tr("Applications associées")).open(&mut open).show(ctx, |ui| {
                ui.label(tr("Noms d'exécutables (ex. obs64.exe) associés à ce profil."));
                let apps = &mut self.config.profiles[self.config.active].apps;
                let mut changed = false;
                let mut remove = None;
                for (i, name) in apps.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        changed |= ui.text_edit_singleline(name).changed();
                        if ui.small_button(tr("Supprimer")).clicked() { remove = Some(i); }
                    });
                }
                if let Some(i) = remove { apps.remove(i); changed = true; }
                if ui.button(tr("Ajouter une application")).clicked() { apps.push(String::new()); changed = true; }
                if ui.button(tr("Ajouter l'application au premier plan dans 3 s")).clicked() {
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
        if self.show_hardware_test {
            let mut open=true;
            egui::Window::new(tr("Tester mon périphérique")).open(&mut open).default_width(540.0).resizable(true).show(ctx,|ui| {
                let p=theme::palette(ui.ctx());
                ui.label(RichText::new(self.device_label()).strong());
                theme::status_badge(ui, &self.status, if self.gate.connected { GREEN } else { theme::WARNING });
                ui.label(tr("Ce test met les macros en pause et remplace temporairement les images. Ferme Elgato / Stream Dock / FIFINE Control Deck avant de commencer."));
                let active=self.hardware_test.as_ref().is_some_and(|t|t.active);
                if ui.add_enabled(self.gate.connected && !active,theme::secondary_button(tr("Lancer le test"))).clicked() {
                    self.engine.stop();
                    self.gate.enabled=false;
                    self.long_press.clear();
                    let grid=self.grid();
                    self.hardware_test=Some(crate::devices::test_assistant::Session::new(grid.keys,grid.cols));
                    if self.device.send(Command::TestStart).is_err() { self.fail(tr("Service HID arrêté").into()); if let Some(t)=&mut self.hardware_test { t.active=false; } }
                }
                if active && ui.add(theme::danger_button(tr("Arrêter le test"))).clicked() {
                    let _=self.device.send(Command::TestStop);
                    if let Some(t)=&mut self.hardware_test { t.active=false; }
                }
                if let Some(t)=&mut self.hardware_test {
                    ui.label(trf("Appuie sur le numéro {next} / {keys} (dans l’ordre, sans action exécutée).", &[("next", &(t.next+1).min(t.keys)), ("keys", &t.keys)]));
                    ui.label(RichText::new(trf("{checked} touche(s) vérifiée(s), {errors} erreur(s)", &[("checked", &t.next), ("errors", &t.mismatches.len())])).color(p.muted));
                    ui.label(tr("Les numéros sont-ils à l’endroit ?"));
                    ui.horizontal(|ui| { ui.selectable_value(&mut t.orientation,Some(true),tr("Oui")); ui.selectable_value(&mut t.orientation,Some(false),tr("Non")); });
                    ui.label(&t.brightness);
                    if t.brightness.contains("100 %") { ui.label(tr("As-tu vu le passage à 30 %, puis à 100 % ? Indique-le dans ton retour au support.")); }
                }
                let result=self.hardware_test.as_ref().map(|t|t.result()).unwrap_or_else(||"Test non lancé.\n".into());
                let log=crate::devices::report::latest(&self.path).ok().flatten().and_then(|p|std::fs::read_to_string(p).ok()).unwrap_or_else(||"Journal périphérique indisponible\n".into());
                let text=format!("Rapport de test NovyDeck\nModèle : {}\nStatut : {}\n\nRésultat du test\n{}\nJournal périphérique\n{}",self.device_label(),self.status,result,log);
                ui.horizontal(|ui| {
                    if ui.add(theme::secondary_button(tr("Copier"))).clicked() { ui.ctx().copy_text(text.clone()); }
                    if ui.add(theme::secondary_button(tr("Enregistrer"))).clicked() {
                        if let Some(path)=rfd::FileDialog::new().add_filter("Texte", &["txt"]).set_file_name("rapport-test-deckforge.txt").save_file() {
                            if let Err(e)=std::fs::write(path.with_extension("txt"),&text) {self.fail(trf("Enregistrement impossible : {error}", &[("error", &e)]));}
                        }
                    }
                });
                egui::ScrollArea::vertical().max_height(180.0).show(ui,|ui|{ui.monospace(&text);});
            });
            if !open {
                if self.hardware_test.as_ref().is_some_and(|t|t.active) { let _=self.device.send(Command::TestStop); if let Some(t)=&mut self.hardware_test { t.active=false; } }
                self.show_hardware_test=false;
            }
        }
        if self.show_device_report {
            let mut open = true;
            egui::Window::new(tr("Rapport périphérique")).open(&mut open)
                .default_width(750.0).default_height(500.0).resizable(true).show(ctx, |ui| {
                    let folder = crate::devices::report::directory(&self.path);
                    match crate::devices::report::latest(&self.path) {
                        Ok(Some(path)) => match std::fs::read_to_string(&path) {
                            Ok(text) => {
                                ui.horizontal(|ui| {
                                    if ui.add(theme::secondary_button(tr("Copier"))).clicked() { ui.ctx().copy_text(text.clone()); }
                                    if ui.add(theme::secondary_button(tr("Ouvrir le dossier"))).clicked() {
                                        if let Err(e) = std::process::Command::new("explorer.exe").arg(&folder).spawn() { self.fail(trf("Dossier inaccessible : {error}", &[("error", &e)])); }
                                    }
                                    if ui.add(theme::secondary_button(tr("Enregistrer sous…"))).clicked() {
                                        if let Some(dest) = rfd::FileDialog::new().add_filter("Texte", &["txt"]).set_file_name("rapport-peripherique.txt").save_file() {
                                            if let Err(e) = std::fs::write(dest.with_extension("txt"), &text) { self.fail(trf("Enregistrement impossible : {error}", &[("error", &e)])); }
                                        }
                                    }
                                });
                                egui::ScrollArea::vertical().show(ui, |ui| { ui.monospace(&text); });
                            }
                            Err(e) => { ui.label(trf("Lecture du rapport impossible : {error}", &[("error", &e)])); }
                        },
                        Ok(None) => { ui.label(tr("Aucun rapport disponible. Lance une connexion du périphérique.")); },
                        Err(e) => { ui.label(trf("Recherche du rapport impossible : {error}", &[("error", &e)])); }
                    }
                });
            self.show_device_report = open;
        }
        if self.settings {
            let mut open = true;
            let d6 = self.is_d6();
            egui::Window::new(trf("Réglages — {device}", &[("device", &self.device_name())])).id(egui::Id::new("device_settings"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(460.0)
                .show(ctx, |ui| {
                    // Onglets : la fenêtre reste courte et tient sur un écran de portable.
                    ui.horizontal(|ui| {
                        for (i, (ico, label)) in [(Ico::Palette, tr("Général")), (Ico::Device, tr("Périphérique")), (Ico::Diagnostic, "Diagnostic")].into_iter().enumerate() {
                            if iconset::tab(ui, self.settings_tab == i as u8, ico, RichText::new(label).size(15.0)).clicked() {
                                self.settings_tab = i as u8;
                            }
                        }
                    });
                    ui.separator();
                    motion::fade(ui, "settings_tab", self.settings_tab, 0.15, self.animations());
                    let help = |text: &str| RichText::new(text).size(12.0).color(p.muted);
                    match self.settings_tab {
                        0 => {
                            iconset::heading(ui, Ico::Palette, RichText::new(tr("Thème de l'interface")).size(17.0).strong());
                            ui.add_space(4.0);
                            let mut theme_changed = false;
                            egui::Grid::new("settings_theme").num_columns(2).spacing([14.0, 10.0]).show(ui, |ui| {
                                ui.label(tr("Style"));
                                let current = theme::SCHEMES.iter().find(|s| s.0 == self.preferences.scheme)
                                    .or_else(|| theme::SCHEMES.iter().find(|s| s.2 == self.preferences.dark))
                                    .map_or(tr("Bleu nuit"), |s| theme::display_color_name(s.1));
                                egui::ComboBox::from_id_salt("theme_scheme").selected_text(current).show_ui(ui, |ui| {
                                    for (id, label, dark) in theme::SCHEMES {
                                        if ui.selectable_label(self.preferences.scheme == id, format!("{}{}", theme::display_color_name(label), if dark { tr(" (sombre)") } else { tr(" (clair)") })).clicked() {
                                            self.preferences.scheme = id.into();
                                            self.preferences.dark = dark;
                                            theme_changed = true;
                                        }
                                    }
                                });
                                ui.end_row();
                                ui.label(tr("Accent"));
                                ui.horizontal(|ui| {
                                    for (id, label, color) in theme::ACCENTS {
                                        let chosen = self.preferences.accent == id || (self.preferences.accent.is_empty() && id == "bleu");
                                        let (r, response) = ui.allocate_exact_size(Vec2::splat(26.0), egui::Sense::click());
                                        ui.painter().circle_filled(r.center(), 10.0, color);
                                        if chosen { ui.painter().circle_stroke(r.center(), 12.5, egui::Stroke::new(2.0_f32, p.ink)); }
                                        if response.on_hover_text(theme::display_color_name(label)).clicked() {
                                            self.preferences.accent = id.into();
                                            theme_changed = true;
                                        }
                                    }
                                });
                                ui.end_row();
                            });
                            if ui.checkbox(&mut self.preferences.reduce_motion, tr("Réduire les animations"))
                                .on_hover_text(tr("Coupe l'intro du démarrage et les fondus de page ; l'indicateur de chargement reste."))
                                .changed()
                            {
                                self.save_preferences();
                            }
                            if theme_changed {
                                theme::set_theme(&self.preferences.scheme, &self.preferences.accent);
                                theme::apply(ctx, self.preferences.dark);
                                self.save_preferences();
                            }
                            ui.add_space(10.0);
                            ui.separator();
                            iconset::heading(ui, Ico::Robot, RichText::new(tr("Assistant IA")).size(17.0).strong());
                            ui.label(help(tr("IA locale sur ce PC et IA connectée (Claude, ChatGPT…) se règlent au même endroit.")));
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new(self.ai_status_label()).strong());
                                if ui.button(tr("Configurer…")).clicked() {
                                    self.assistant_tab = 0;
                                    self.assistant_settings = true;
                                }
                            });
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label(tr("Langue"));
                                let current = crate::i18n::languages().into_iter()
                                    .find(|(code, _)| *code == self.preferences.language)
                                    .map(|(_, name)| name).unwrap_or(tr("Langue de Windows"));
                                egui::ComboBox::from_id_salt("settings_language")
                                    .selected_text(current).show_ui(ui, |ui| {
                                        if ui.selectable_value(&mut self.preferences.language, String::new(), tr("Langue de Windows")).changed() {
                                            crate::i18n::set_language(crate::i18n::resolve(&self.preferences.language));
                                            self.save_preferences();
                                        }
                                        for (code, name) in crate::i18n::languages() {
                                            if ui.selectable_value(&mut self.preferences.language, code.to_owned(), name).changed() {
                                                crate::i18n::set_language(crate::i18n::resolve(&self.preferences.language));
                                                self.save_preferences();
                                            }
                                        }
                                    });
                            });
                            if ui.link(tr("Aider à traduire NovyDeck")).clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab("https://github.com/HatiFlash/NovyDeck/blob/main/docs/TRANSLATING.md"));
                            }
                            ui.separator();
                            let mut check = self.preferences.update_check.unwrap_or(false);
                            if ui.checkbox(&mut check, tr("Vérifier les mises à jour au démarrage")).changed() {
                                self.preferences.update_check = Some(check);
                                self.save_preferences();
                            }
                        }
                        1 => {
                            iconset::heading(ui, Ico::Brightness, RichText::new(tr("Affichage")).size(17.0).strong());
                            ui.add_space(4.0);
                            let mut edited = false;
                            let mut auto_profile = false;
                            egui::Grid::new("settings_device").num_columns(2).spacing([14.0, 12.0]).show(ui, |ui| {
                                ui.label(tr("Luminosité"));
                                edited |= ui.add(egui::Slider::new(&mut self.config.brightness, 0..=100).suffix(" %")).changed();
                                ui.end_row();
                                ui.label(tr("Veille après"));
                                edited |= ui.add(egui::Slider::new(&mut self.config.sleep_after_min, 0..=240).suffix(" min")).on_hover_text(tr("0 = jamais")).changed();
                                ui.end_row();
                                ui.label(tr("Luminosité en veille"));
                                edited |= ui.add(egui::Slider::new(&mut self.config.sleep_brightness, 0..=100).suffix(" %")).changed();
                                ui.end_row();
                                ui.label(tr("Appui long"));
                                edited |= ui.add(egui::Slider::new(&mut self.config.long_press_ms, 250..=2000).suffix(" ms")).changed();
                                ui.end_row();
                                ui.label(tr("Images/s max"));
                                edited |= ui.add(egui::Slider::new(&mut self.config.animation_fps, 1..=15)).changed();
                                ui.end_row();
                            });
                            ui.label(help(tr("Veille : 0 = jamais. Une touche avec appui long attend ce délai ; sans relâchement détecté, elle lance l’action longue.")));
                            ui.add_space(6.0);
                            if ui.checkbox(&mut self.config.auto_profile, tr("Changer de profil selon l'application")).changed() { edited = true; auto_profile = true; }
                            edited |= ui.checkbox(&mut self.config.d6_animations, tr("Animations GIF et vidéo sur les touches")).changed();
                            ui.label(help(tr("Les GIF et les vidéos converties sont joués sur le périphérique, une image à la fois.")));
                            if edited { self.edited(); }
                            if auto_profile { self.update_auto_profile(); }
                            if d6 {
                                ui.add_space(6.0);
                                egui::CollapsingHeader::new(tr("Compatibilité avancée")).show(ui, |ui| {
                                    if ui.checkbox(&mut self.config.flip_input_rows, tr("Inverser les lignes des appuis")).changed() {
                                        self.edited();
                                    }
                                    ui.horizontal(|ui| {
                                        ui.label("Images");
                                        if ui.selectable_value(&mut self.config.image_size, 126, "126 px").changed()
                                            || ui.selectable_value(&mut self.config.image_size, 112, "112 px").changed()
                                        {
                                            self.edited();
                                        }
                                    });
                                    ui.label(help(tr("Ton D6 a été validé en 126 px, sans inversion des appuis.")));
                                });
                            }
                        }
                        _ => {
                            iconset::heading(ui, Ico::Diagnostic, RichText::new(tr("Diagnostic du clignotement")).size(17.0).strong());
                            let running=self.flicker.as_ref().is_some_and(|session|session.active());
                            if ui.button(if running {tr("Arrêter le diagnostic")} else {tr("Démarrer le diagnostic (90 s max)")}).clicked() {
                                if running { if let Some(session)=&self.flicker {session.stop();} }
                                else { match crate::flicker_diagnostic::Session::start(&self.path) {
                                    Ok(session)=> {self.flicker_path=Some(session.path.clone()); self.flicker=Some(session);}
                                    Err(e)=> self.fail(trf("Diagnostic indisponible : {error}", &[("error", &e)])),
                                }}
                            }
                            ui.label(help(tr("Capture locale sans réseau, limitée à 90 s et 6 Mo. Survole l'aperçu puis les réglages avant d'arrêter.")));
                            if let Some(path)=&self.flicker_path {
                                let display=path.to_string_lossy().into_owned();
                                ui.label(trf("Fichier : {display}", &[("display", &display)]));
                                if ui.button(tr("Copier le chemin du diagnostic")).clicked() {ui.ctx().copy_text(display);}
                            }
                            if let Some(session)=&self.flicker { ui.label(trf("{state} · pertes : {losses}", &[("state", &if session.active(){tr("En cours")}else{tr("Arrêté")}), ("losses", &session.dropped())])); }
                            ui.add_space(10.0);
                            ui.separator();
                            iconset::heading(ui, Ico::Keyboard, RichText::new(tr("Diagnostic des touches")).size(17.0).strong());
                            if ui.checkbox(&mut self.gate.inspect, tr("Sélectionner les touches sans exécuter d’action")).changed() {
                                self.engine.stop();
                            }
                            ui.label(help(tr("Le mode actif est indiqué dans la barre supérieure.")));
                        }
                    }
                });
            self.settings = open;
        }
        if self.rename {
            let mut open = true;
            egui::Window::new(tr("Ce profil"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    let old = self.config.profiles[self.config.active].name.clone();
                    ui.label(tr("Nom du profil"));
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
                            egui::Button::new(tr("Dupliquer ce profil")),
                        )
                        .clicked()
                    {
                        self.add_profile(true);
                    }
                });
            self.rename = open;
        }
        about::hardware_report_window(ctx, self);
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
            || plugin_market_ui::is_open()
            || self.show_assets
            || self.show_desktop_settings
            || self.page_manager
            || self.delete_page.is_some()
            || self.recording.is_some()
            || !self.import_reports.is_empty()
            || self.crop_session.is_some()
            || self.settings
            || self.show_hardware_test
            || self.show_device_report
            || self.hardware_report.is_some()
            || self.show_saved_variables
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
        self.selected = self.selected.min(self.device_keys() - 1);
        self.shortcuts(ctx);
        self.frames += 1;
        self.key_previews.begin_frame();
        self.screenshot(ctx);
        self.header(ctx);
        self.footer(ctx);
        if self.show_workflow {
            self.workflow(ctx);
        } else if plugin_market_ui::is_open() {
            // Le magasin remplace la zone centrale ; l'en-tête et le pied de page restent.
            self.plugin_market(ctx);
        } else {
            self.sidebar(ctx);
            self.catalog(ctx);
            self.editor(ctx);
            self.deck(ctx);
        }
        let logo = theme::intro_logo_texture(ctx);
        motion::intro(ctx, &logo, self.animations());
        if self.drag.moving() {
            if let (Some((item, _)), Some(pos)) = (&self.drag.active, self.drag.position) {
                let label = match item {
                    DragItem::Bind(k) | DragItem::Action(k) => k.label.clone(),
                    DragItem::Position(_, _, i) => trf("Touche {key}", &[("key", &format!("{:02}", i + 1))]),
                    DragItem::NewFolder => tr("Nouveau dossier").into(),
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
        if self.hide_frames > 0 && !self.open_requested {
            // eframe 0.31 rend la fenêtre visible après la première image, même avec `with_visible(false)`.
            self.hide_frames -= 1;
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
        if context.starts_with("novydeck-block-") && matches!(name,"setTitle"|"setImage"|"setState"|"showOk"|"showAlert"|"setSettings"|"getSettings") {
            let Some((index,path))=crate::plugin_runtime::virtual_step_path(&self.applied,context) else {return};
            let Some(crate::model::Step::PluginAction { plugin: owner, action, settings, .. })=
                extras::step_mut(&mut self.applied.keys_mut()[index].steps,&path) else {return};
            // setSettings/getSettings du SDK ne portent pas le champ `action` : le contexte virtuel suffit.
            if owner!=plugin || message["action"].as_str().is_some_and(|a|a!=action.as_str()) {return;}
            if name=="getSettings" {
                let v=crate::plugin_runtime::Visible {context:context.into(),plugin:owner.clone(),action:action.clone(),index:0,settings:settings.clone(),state:0};
                if let Some(manager)=&self.plugins {manager.send(plugin,app_settings_event(&v,&crate::plugin_runtime::Device::virtual_device(),settings,None));}
            } else if name=="setSettings" {
                if let Some(new)=message["payload"].as_object() {
                    let new=serde_json::Value::Object(new.clone());
                    *settings=new.clone();
                    if self.config.active==self.applied.active && self.config.page==self.applied.page && !self.gate.dirty {
                        if let Some(crate::model::Step::PluginAction { settings, .. })=extras::step_mut(&mut self.config.keys_mut()[index].steps,&path) {
                            *settings=new.clone();
                            if let Err(e)=store::save(&self.path,&self.config){self.fail(trf("Réglages du plugin non sauvegardés : {error}", &[("error", &e)]));}
                            else if let Some(manager)=&self.plugins {manager.virtual_settings(context,new);}
                        }
                    }
                }
            }
            return;
        }
        let pi=message["__pi"].as_str();
        let sdk_device=self.device_model.as_ref().map_or_else(crate::plugin_runtime::Device::virtual_device,|d|crate::plugin_runtime::Device::from_grid(d.name.clone(),d.cols,d.rows));
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
                if let (Some(v),Some(settings))=(visible,message["payload"].as_object()){
                    if self.config.active==self.applied.active && self.config.page==self.applied.page && !self.gate.dirty {
                        if let Some(binding)=self.config.keys_mut()[v.index].plugin.as_mut(){binding.settings=json!(settings);}
                        if let Some(binding)=self.applied.keys_mut()[v.index].plugin.as_mut(){binding.settings=json!(settings);}
                        if let Err(e)=store::save(&self.path,&self.config){self.fail(trf("Réglages du plugin non sauvegardés : {error}", &[("error", &e)]));}
                        else if let Some(manager)=&mut self.plugins {manager.sync_settings(&self.applied,&v.context,pi.is_some());}
                    }
                }
            },
            "getSettings"=>{if let (Some(v),Some(manager))=(visible,&self.plugins){manager.send(plugin,app_settings_event(&v,&sdk_device,&v.settings,pi));}},
            "setGlobalSettings"=>{let settings=&message["payload"];if settings.is_object(){let path=self.path.with_file_name("plugin-settings.json");let mut all=std::fs::read(&path).ok().and_then(|b|serde_json::from_slice::<serde_json::Value>(&b).ok()).filter(|v|v.is_object()).unwrap_or_else(||json!({}));all[plugin]=settings.clone();if let Err(e)=std::fs::write(path,serde_json::to_vec_pretty(&all).unwrap_or_default()){self.fail(trf("Réglages globaux non sauvegardés : {error}", &[("error", &e)]));}else if let Some(manager)=&self.plugins {manager.send(plugin,json!({"event":"didReceiveGlobalSettings","payload":{"settings":settings},"__pi":"*"}));if pi.is_some(){manager.send(plugin,json!({"event":"didReceiveGlobalSettings","payload":{"settings":settings}}));}}}},
            "getGlobalSettings"=>{let all=std::fs::read(self.path.with_file_name("plugin-settings.json")).ok().and_then(|b|serde_json::from_slice::<serde_json::Value>(&b).ok()).unwrap_or_else(||json!({}));if let Some(manager)=&self.plugins {manager.send(plugin,json!({"event":"didReceiveGlobalSettings","payload":{"settings":all.get(plugin).cloned().unwrap_or_else(||json!({}))},"__pi":pi}));}},
            "openUrl"=>{if let Some(url)=message["payload"]["url"].as_str().filter(|u|u.starts_with("https://")||u.starts_with("http://")){
                if let Some(effects)=&mut self.plugin_external_effects { effects.push(format!("openUrl: {url}")); }
                else { let _=std::process::Command::new("rundll32.exe").arg("url.dll,FileProtocolHandler").arg(url).spawn(); }
            }},
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
// Réponse App à setSettings/getSettings ; même contrat que les événements du runtime.
fn app_settings_event(v:&crate::plugin_runtime::Visible,device:&crate::plugin_runtime::Device,settings:&serde_json::Value,pi:Option<&str>)->serde_json::Value {
    let mut current=v.clone();
    current.settings=settings.clone();
    let mut message=crate::plugin_runtime::event("didReceiveSettings",&current,device);
    if let Some(id)=pi {message["__pi"]=serde_json::json!(id);}
    message
}
#[cfg(test)]
#[test]
fn app_did_receive_settings_contract() {
    use serde_json::json;
    let v=crate::plugin_runtime::Visible{context:"ctx".into(),plugin:"p".into(),action:"a".into(),index:7,settings:json!({"old":true}),state:1};
    let device=crate::plugin_runtime::Device::virtual_device();
    let set=app_settings_event(&v,&device,&json!({"new":true}),None);
    assert_eq!(set["payload"]["settings"],json!({"new":true}));
    assert_eq!(set["payload"]["resources"],json!({}));
    assert_eq!(set["payload"]["controller"],"Keypad");
    assert!(set.get("__pi").is_none());
    let get=app_settings_event(&v,&device,&v.settings,Some("inspector"));
    assert_eq!(get["payload"]["settings"],json!({"old":true}));
    assert_eq!(get["payload"]["resources"],json!({}));
    assert_eq!(get["payload"]["coordinates"],json!({"column":2,"row":1}));
    assert_eq!(get["__pi"],"inspector");
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
        ("foreground_app", tr("Application au premier plan")), ("app_running", tr("Application ouverte (arrière-plan inclus)")),
        ("window_open", tr("Fenêtre ouverte")), ("app_audio_playing", tr("Session audio active")),
        ("app_muted", tr("Application muette")), ("app_volume_compare", tr("Volume d’application")),
        ("device_muted", tr("Périphérique muet")), ("device_volume_compare", tr("Volume du périphérique")),
        ("audio_device_present", tr("Périphérique audio actif")), ("default_output_is", tr("Sortie par défaut")),
        ("variable_compare", tr("Variable")), ("not", tr("NON")), ("all", tr("Toutes (ET)")), ("any", tr("Au moins une (OU)"))
    ];
    let mut selected = condition.kind().to_owned();
    egui::ComboBox::from_id_salt(("condition_kind", depth)).selected_text(variants.iter().find(|(id, _)| *id == condition.kind()).map(|(_, label)| *label).unwrap_or(condition.label())).show_ui(ui, |ui| {
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
            ui.label(tr("Nom d’exécutable (.exe) :"));
            changed |= condition_exe(ui, exe);
        }
        C::WindowOpen { title } => { ui.label(tr("Le titre contient :")); changed |= ui.text_edit_singleline(title).changed(); }
        C::AppAudioPlaying { exe } | C::AppMuted { exe } => { changed |= condition_audio_exe(ui, exe); }
        C::AppVolumeCompare { exe, op, value } => {
            changed |= condition_audio_exe(ui, exe);
            changed |= condition_operator(ui, op);
            changed |= ui.add(egui::Slider::new(value, 0..=100).text("%" )).changed();
        }
        C::DeviceMuted { input, device } | C::DeviceVolumeCompare { input, device, .. } => {
            changed |= ui.checkbox(input, tr("Microphone (sinon sortie)")).changed();
            egui::ComboBox::from_id_salt("condition_device").selected_text(if device == "default" { tr("Par défaut") } else { device.as_str() }).show_ui(ui, |ui| {
                changed |= ui.selectable_value(device, "default".into(), tr("Par défaut")).changed();
                for endpoint in audio.devices.iter().filter(|d| d.input == *input) {
                    changed |= ui.selectable_value(device, endpoint.id.clone(), &endpoint.name).changed();
                }
            });
            egui::CollapsingHeader::new(tr("Options avancées")).id_salt("condition_device_id").show(ui, |ui| {
                changed |= ui.text_edit_singleline(device).changed();
            });
        }
        C::AudioDevicePresent { name_contains, input } => {
            changed |= ui.checkbox(input, tr("Microphone (sinon sortie)")).changed();
            let options: Vec<_> = audio.devices.iter().filter(|d| d.input == *input).collect();
            egui::ComboBox::from_id_salt("condition_device_name").selected_text(name_contains.as_str()).show_ui(ui, |ui| {
                for endpoint in options { changed |= ui.selectable_value(name_contains, endpoint.name.clone(), &endpoint.name).changed(); }
            });
            ui.label(tr("Le nom contient :")); changed |= ui.text_edit_singleline(name_contains).changed();
        }
        C::DefaultOutputIs { name_contains } => {
            egui::ComboBox::from_id_salt("condition_default_output").selected_text(name_contains.as_str()).show_ui(ui, |ui| {
                for endpoint in audio.devices.iter().filter(|d| !d.input) { changed |= ui.selectable_value(name_contains, endpoint.name.clone(), &endpoint.name).changed(); }
            });
            ui.label(tr("Le nom contient :")); changed |= ui.text_edit_singleline(name_contains).changed();
        }
        C::VariableCompare { name, op, value } => {
            ui.label(tr("Nom de variable")); changed |= ui.text_edit_singleline(name).changed();
            egui::ComboBox::from_id_salt("variable_operator").selected_text(match op.as_str() { "equals" => tr("Égale"), "not_equals" => tr("Différente"), "contains" => tr("Contient"), "greater" => tr("Supérieure"), "less" => tr("Inférieure"), _ => op.as_str() }).show_ui(ui, |ui| {
                for (id, label) in [("equals", tr("Égale")), ("not_equals", tr("Différente")), ("contains", tr("Contient")), ("greater", tr("Supérieure")), ("less", tr("Inférieure"))] {
                    changed |= ui.selectable_value(op, id.into(), label).changed();
                }
            });
            ui.label(tr("Valeur")); changed |= ui.text_edit_singleline(value).changed();
        }
        C::Not { condition } => { ui.indent("not_condition", |ui| { changed |= edit_condition(ui, condition, audio, depth + 1); }); }
        C::All { conditions } | C::Any { conditions } => {
            let mut remove = None;
            for (i, child) in conditions.iter_mut().enumerate() {
                ui.push_id(i, |ui| { ui.horizontal(|ui| { ui.label(trf("Condition {number}", &[("number", &(i + 1))]));
                    if ui.small_button(tr("Retirer")).clicked() { remove = Some(i); }
                });
                ui.indent(i, |ui| { changed |= edit_condition(ui, child, audio, depth + 1); }); });
            }
            if let Some(i) = remove { if conditions.len() > 1 { conditions.remove(i); changed = true; } }
            if depth < 8 && conditions.len() < 32 && ui.button(tr("+ Ajouter une condition")).clicked() { conditions.push(C::default()); changed = true; }
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
    egui::ComboBox::from_id_salt("condition_audio_exe").selected_text(if exe == "foreground" { tr("Application au premier plan") } else { exe.as_str() }).show_ui(ui, |ui| {
        changed |= ui.selectable_value(exe, "foreground".into(), tr("Application au premier plan")).changed();
        for name in crate::conditions::running_exes() { changed |= ui.selectable_value(exe, name.clone(), name).changed(); }
    });
    ui.label(tr("Ou saisir un exécutable (.exe) :"));
    changed | ui.text_edit_singleline(exe).changed()
}
fn condition_operator(ui: &mut egui::Ui, op: &mut String) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt("volume_operator").selected_text(if op == "above" { tr("Au-dessus de") } else { tr("En dessous de") }).show_ui(ui, |ui| {
        changed |= ui.selectable_value(op, "above".into(), tr("Au-dessus de")).changed();
        changed |= ui.selectable_value(op, "below".into(), tr("En dessous de")).changed();
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
        Step::PluginAction { .. } => {
            ui.label(tr("Action fournie par un plugin installé."));
        }
        Step::KeyHold { keys, action, hold_ms } => {
            ui.label(tr("Touches (ex. CTRL+SHIFT+A)"));
            changed |= ui.text_edit_singleline(keys).changed();
            egui::ComboBox::from_id_salt("key_hold_action").selected_text(if action == "up" { tr("Relâcher") } else { tr("Maintenir") }).show_ui(ui, |ui| {
                changed |= ui.selectable_value(action, "down".into(), tr("Maintenir")).changed();
                changed |= ui.selectable_value(action, "up".into(), tr("Relâcher")).changed();
            });
            if action == "down" {
                ui.collapsing(tr("Options avancées"), |ui| {
                    ui.label(tr("Durée (ms) : 0 = jusqu’au bloc Relâcher ou fin de macro"));
                    changed |= ui.add(egui::DragValue::new(hold_ms).range(0..=3_600_000)).changed();
                });
            } else if *hold_ms != 0 { *hold_ms = 0; changed = true; }
        }
        Step::MouseClick { button, double, x, y, relative_to } => {
            egui::ComboBox::from_id_salt("mouse_button").selected_text(match button.as_str() { "left" => tr("Gauche"), "right" => tr("Droit"), _ => tr("Milieu") }).show_ui(ui, |ui| {
                for (id, label) in [("left", tr("Gauche")), ("right", tr("Droit")), ("middle", tr("Milieu"))] { changed |= ui.selectable_value(button, id.into(), label).changed(); }
            });
            changed |= ui.checkbox(double, tr("Double clic")).changed();
            ui.collapsing(tr("Options avancées"), |ui| {
                let mut positioned = x.is_some();
                if ui.checkbox(&mut positioned, tr("Déplacer avant de cliquer")).changed() { *x = positioned.then_some(0); *y = positioned.then_some(0); changed = true; }
                if positioned {
                    mouse_origin_editor(ui, relative_to, &mut changed);
                    ui.horizontal(|ui| { ui.label("X"); changed |= ui.add(egui::DragValue::new(x.as_mut().unwrap()).range(-32768..=32767)).changed(); ui.label("Y"); changed |= ui.add(egui::DragValue::new(y.as_mut().unwrap()).range(-32768..=32767)).changed(); });
                }
            });
        }
        Step::MouseMove { x, y, relative, relative_to } => {
            ui.horizontal(|ui| { ui.label("X"); changed |= ui.add(egui::DragValue::new(x).range(-32768..=32767)).changed(); ui.label("Y"); changed |= ui.add(egui::DragValue::new(y).range(-32768..=32767)).changed(); });
            ui.collapsing(tr("Options avancées"), |ui| {
                if *relative { *relative_to = "cursor".into(); *relative = false; changed = true; }
                mouse_origin_editor(ui, relative_to, &mut changed);
            });
        }
        Step::MouseScroll { amount, horizontal } => {
            ui.label(tr("Défilement (crans)")); changed |= ui.add(egui::DragValue::new(amount).range(-100..=100)).changed();
            ui.collapsing(tr("Options avancées"), |ui| { changed |= ui.checkbox(horizontal, tr("Horizontal")).changed(); });
        }
        Step::PlaySound { path, volume } => { ui.label(tr("Fichier WAV ou MP3")); changed |= ui.text_edit_singleline(path).changed(); ui.label(tr("Volume")); changed |= ui.add(egui::Slider::new(volume, 0..=100)).changed(); }
        Step::StopSounds => { ui.label(tr("Arrête tous les sons lancés par NovyDeck.")); }
        Step::Toggle { .. } => { ui.label(tr("Alterne les états 1 et 2 à chaque appui. Modifie leurs blocs dans l’Atelier.")); }
        Step::UdpSend { host, port, text } => { ui.label(tr("Message : accepte {nom}, {date}, {heure}, {horodatage}, {presse_papiers}.")); ui.label(tr("Hôte UDP")); changed |= ui.text_edit_singleline(host).changed(); ui.label(tr("Port")); changed |= ui.add(egui::DragValue::new(port).range(1..=65535)).changed(); ui.label(tr("Message")); changed |= ui.text_edit_multiline(text).changed(); }
        Step::HttpRequest { method, url, body, content_type } => { ui.label(tr("Adresse et corps : {nom}, {date}, {heure}, {horodatage}, {presse_papiers}.")); ui.label(tr("Méthode")); egui::ComboBox::from_id_salt("http_method").selected_text(method.as_str()).show_ui(ui, |ui| { for choice in ["GET", "POST", "PUT"] { changed |= ui.selectable_value(method, choice.into(), choice).changed(); } }); ui.label(tr("Adresse HTTP (sans HTTPS)")); changed |= ui.text_edit_singleline(url).changed(); ui.label(tr("Corps")); changed |= ui.text_edit_multiline(body).changed(); ui.label(tr("Type de contenu")); changed |= ui.text_edit_singleline(content_type).changed(); }
        Step::D6Brightness { mode, value } => {
            ui.label(tr("Luminosité du périphérique"));
            egui::ComboBox::from_id_salt("d6_brightness_mode").selected_text(match mode.as_str() { "up" => tr("Augmenter"), "down" => tr("Diminuer"), _ => tr("Régler à") }).show_ui(ui, |ui| {
                for (id, label) in [("set", tr("Régler à")), ("up", tr("Augmenter")), ("down", tr("Diminuer"))] { changed |= ui.selectable_value(mode, id.into(), label).changed(); }
            });
            changed |= ui.add(egui::Slider::new(value, 0..=100).text("%" )).changed();
        }
        Step::DeviceVolume { input, device, action, step, level } => {
            ui.label(tr("Volume du périphérique (pas celui d'une application)"));
            changed |= ui.checkbox(input, tr("Microphone (sinon sortie)")).changed();
            let options: Vec<_> = audio.devices.iter().filter(|d| d.input == *input).collect();
            let selected = if device == "default" { tr("Par défaut").to_string() }
                else { options.iter().find(|d| d.id == *device).map(|d| d.name.clone()).unwrap_or_else(|| device.clone()) };
            egui::ComboBox::from_id_salt("device_volume_target").selected_text(selected).show_ui(ui, |ui| {
                changed |= ui.selectable_value(device, "default".into(), tr("Par défaut")).changed();
                for endpoint in &options {
                    changed |= ui.selectable_value(device, endpoint.id.clone(), &endpoint.name).changed();
                }
            });
            ui.label(tr("Action"));
            egui::ComboBox::from_id_salt("device_volume_action").selected_text(match action.as_str() {
                "up" => tr("Augmenter"), "down" => tr("Diminuer"), "set" => tr("Régler à"), "mute" => tr("Couper"), "unmute" => tr("Rétablir"), _ => tr("Basculer muet"),
            }).show_ui(ui, |ui| {
                for (id, label) in [("up", tr("Augmenter")), ("down", tr("Diminuer")), ("set", tr("Régler à")), ("mute", tr("Couper")), ("unmute", tr("Rétablir")), ("toggle_mute", tr("Basculer muet"))] {
                    changed |= ui.selectable_value(action, id.into(), label).changed();
                }
            });
            if action == "up" || action == "down" { changed |= ui.add(egui::Slider::new(step, 1..=100).text(tr("Pas (points)"))).changed(); }
            if action == "set" { changed |= ui.add(egui::Slider::new(level, 0..=100).text(tr("Niveau (%)"))).changed(); }
            egui::CollapsingHeader::new(tr("Options avancées")).id_salt("device_volume_advanced").show(ui, |ui| {
                ui.label(tr("ID du périphérique (ou default) :"));
                changed |= ui.text_edit_singleline(device).changed();
            });
        }
        Step::Clipboard { op, value, name } => {
            ui.label(tr("Action sur le presse-papiers"));
            egui::ComboBox::from_id_salt("clipboard_op").selected_text(match op.as_str() {
                "copy_text" => tr("Copier du texte"), "read_to_variable" => tr("Lire dans une variable"),
                "clear" => tr("Vider"), "save" => tr("Sauvegarder le texte"), _ => tr("Restaurer le texte"),
            }).show_ui(ui, |ui| {
                for (id, label) in [("copy_text", tr("Copier du texte")), ("read_to_variable", tr("Lire dans une variable")), ("clear", "Vider"), ("save", tr("Sauvegarder le texte")), ("restore", tr("Restaurer le texte"))] {
                    changed |= ui.selectable_value(op, id.into(), label).changed();
                }
            });
            if op == "copy_text" { ui.label(tr("Texte (accepte {nom})")); changed |= ui.text_edit_multiline(value).changed(); }
            if op == "read_to_variable" { ui.label(tr("Nom de la variable")); changed |= ui.text_edit_singleline(name).changed(); }
            egui::CollapsingHeader::new(tr("Options avancées")).id_salt("clipboard_advanced").show(ui, |ui| {
                ui.label(tr("Sauvegarder et restaurer conservent le texte uniquement pendant cette exécution ; les autres formats ne sont pas restaurés."));
                if op != "copy_text" { ui.label(tr("Valeur enregistrée (utilisée seulement par Copier du texte)")); changed |= ui.text_edit_multiline(value).changed(); }
                if op != "read_to_variable" { ui.label(tr("Nom de variable (utilisé seulement par Lire)")); changed |= ui.text_edit_singleline(name).changed(); }
            });
        }
        Step::WindowOp { target, value, op, x, y, w, h, monitor } => {
            ui.label(tr("Fenêtre ciblée"));
            egui::ComboBox::from_id_salt("window_op_target").selected_text(match target.as_str() { "title" => tr("Titre"), "exe" => tr("Exécutable"), _ => tr("Fenêtre active") }).show_ui(ui, |ui| {
                for (id, label) in [("foreground", tr("Fenêtre active")), ("title", tr("Titre")), ("exe", tr("Exécutable"))] { changed |= ui.selectable_value(target, id.into(), label).changed(); }
            });
            if target != "foreground" { ui.label(if target == "title" { tr("Titre contenant") } else { tr("Nom .exe") }); changed |= ui.text_edit_singleline(value).changed(); }
            ui.label(tr("Action"));
            egui::ComboBox::from_id_salt("window_op_action").selected_text(match op.as_str() {
                "activate" => tr("Activer"), "minimize" => tr("Réduire"), "maximize" => tr("Maximiser"), "restore" => tr("Restaurer"), "move" => tr("Déplacer"), "resize" => tr("Redimensionner"), "move_to_monitor" => tr("Changer d’écran"), "toggle_always_on_top" => tr("Basculer toujours au premier plan"), "always_on_top_on" => tr("Toujours au premier plan"), "close_others" => tr("Fermer les autres fenêtres"), "minimize_others" => tr("Réduire les autres fenêtres"), _ => tr("Retirer du premier plan permanent")
            }).show_ui(ui, |ui| {
                for (id, label) in [("activate", tr("Activer")), ("minimize", tr("Réduire")), ("maximize", tr("Maximiser")), ("restore", tr("Restaurer")), ("move", tr("Déplacer")), ("resize", tr("Redimensionner")), ("move_to_monitor", tr("Changer d’écran")), ("toggle_always_on_top", tr("Basculer toujours au premier plan")), ("always_on_top_on", tr("Toujours au premier plan")), ("always_on_top_off", tr("Retirer du premier plan permanent")), ("close_others", tr("Fermer les autres fenêtres")), ("minimize_others", tr("Réduire les autres fenêtres"))] { changed |= ui.selectable_value(op, id.into(), label).changed(); }
            });
            if op == "move_to_monitor" { changed |= ui.add(egui::DragValue::new(monitor).range(1..=16).prefix(tr("Écran "))).changed(); }
            egui::CollapsingHeader::new(tr("Options avancées")).id_salt("window_op_advanced").show(ui, |ui| {
                if op == "move" { changed |= ui.add(egui::DragValue::new(x).range(-32768..=32767).prefix("X ")).changed(); changed |= ui.add(egui::DragValue::new(y).range(-32768..=32767).prefix("Y ")).changed(); }
                if op == "resize" { changed |= ui.add(egui::DragValue::new(w).range(1..=32767).prefix(tr("Largeur "))).changed(); changed |= ui.add(egui::DragValue::new(h).range(1..=32767).prefix(tr("Hauteur "))).changed(); }
                ui.label(tr("Écran 1 = écran principal ; autres écrans triés de gauche à droite, puis de haut en bas."));
            });
        }
        Step::AppVolume { target, action, step, level, include_children } => {
            ui.label(tr("Cible"));
            let mut foreground = target == "foreground";
            egui::ComboBox::from_id_salt("app_volume_target").selected_text(if foreground { tr("Application au premier plan") } else { tr("Application précise") }).show_ui(ui, |ui| {
                changed |= ui.selectable_value(&mut foreground, true, tr("Application au premier plan")).changed();
                changed |= ui.selectable_value(&mut foreground, false, tr("Application précise")).changed();
            });
            if foreground { if target != "foreground" { *target = "foreground".into(); changed = true; } }
            else {
                if target == "foreground" { *target = "chrome.exe".into(); changed = true; }
                ui.label(tr("Nom du programme (ex. chrome.exe)"));
                changed |= ui.text_edit_singleline(target).changed();
            }
            ui.label(tr("Action"));
            egui::ComboBox::from_id_salt("app_volume_action").selected_text(match action.as_str() {
                "up" => tr("Augmenter"), "down" => tr("Diminuer"), "set" => tr("Régler à"), "mute" => tr("Couper"), "unmute" => tr("Rétablir"), _ => tr("Basculer muet"),
            }).show_ui(ui, |ui| {
                for (id, label) in [("up", tr("Augmenter")), ("down", tr("Diminuer")), ("set", tr("Régler à")), ("mute", tr("Couper")), ("unmute", tr("Rétablir")), ("toggle_mute", tr("Basculer muet"))] {
                    changed |= ui.selectable_value(action, id.into(), label).changed();
                }
            });
            if action == "up" || action == "down" { changed |= ui.add(egui::Slider::new(step, 1..=100).text(tr("Pas (points)"))).changed(); }
            if action == "set" { changed |= ui.add(egui::Slider::new(level, 0..=100).text(tr("Niveau (%)"))).changed(); }
            egui::CollapsingHeader::new(tr("Options avancées")).id_salt("app_volume_advanced").show(ui, |ui| {
                changed |= ui.checkbox(include_children, tr("Inclure les processus enfants")).changed();
            });
        }
        Step::D6Sleep => { ui.label(tr("Met le périphérique en veille maintenant. Un appui le réveille sans lancer la macro.")); }
        Step::CloseWindow => { ui.label(tr("Ferme la fenêtre active (jamais NovyDeck).")); }
        Step::ObsScene { name } => { ui.label(tr("Scène OBS")); changed |= obs_entry(ui, name, &crate::obs_monitor::snapshot().scenes); }
        Step::ObsMute { input, mode } => { ui.label(tr("Entrée audio OBS")); changed |= obs_entry(ui, input, &crate::obs_monitor::snapshot().inputs); obs_choice(ui, mode, &["toggle", "mute", "unmute"], &mut changed); }
        Step::ObsSourceVisibility { scene, source, mode } => { ui.label(tr("Scène puis source OBS")); changed |= obs_entry(ui, scene, &crate::obs_monitor::snapshot().scenes); changed |= obs_entry(ui, source, crate::obs_monitor::snapshot().scene_items.get(scene).map(Vec::as_slice).unwrap_or(&[])); obs_choice(ui, mode, &["toggle", "show", "hide"], &mut changed); }
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
                // Le champ laisse la place au bouton : panneau de propriétés étroit compris.
                let button = ui.fonts(|f| f.layout_no_wrap(tr("Enregistrer…").into(), egui::TextStyle::Button.resolve(ui.style()), Color32::WHITE).size().x)
                    + ui.spacing().button_padding.x * 2.0;
                let width = (ui.available_width() - button - ui.spacing().item_spacing.x).max(60.0);
                changed |= ui.add(egui::TextEdit::singleline(keys).desired_width(width)).changed();
                if ui.button(tr("Enregistrer…")).clicked() {
                    *record = true;
                }
            });
            ui.label(RichText::new("Ex. CTRL+SHIFT+S · ALT+TAB · F13").small());
        }
        Step::Text { text } => {
            ui.label(tr("Gabarit : {nom}, {date}, {heure}, {horodatage}, {presse_papiers} ; {{ pour une accolade."));
            changed |= ui
                .add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(2)
                        .desired_width(ui.available_width()),
                )
                .changed();
        }
        Step::Launch { program, args, focus_if_running } => {
            ui.label(tr("Arguments : {nom}, {date}, {heure}, {horodatage}, {presse_papiers}."));
            ui.horizontal(|ui| {
                changed |= ui.text_edit_singleline(program).changed();
                if ui.small_button("…").clicked() {
                    choose_step_file(tx.clone(),target.clone(),false);
                }
            });
            changed |= ui.checkbox(focus_if_running, tr("Si déjà ouverte, activer au lieu de relancer")).changed();
            ui.label(RichText::new(tr("Arguments : un par ligne, sans guillemets ajoutés")).small());
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
            ui.label(tr("Adresse : accepte {nom}, {date}, {heure}, {horodatage}, {presse_papiers}."));
            changed |= ui.text_edit_singleline(url).changed();
        }
        Step::WaitWindow {
            title,
            timeout_ms,
            focus,
        } => {
            ui.label(tr("Le titre contient (sans distinction majuscule/minuscule)"));
            changed |= ui.text_edit_singleline(title).changed();
            ui.horizontal(|ui| {
                ui.label(tr("Délai max."));
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
                .checkbox(focus, tr("Mettre cette fenêtre au premier plan"))
                .changed();
        }
        Step::Media { command } => {
            let items = [
                ("play_pause", tr("Lecture / pause")),
                ("next", tr("Piste suivante")),
                ("previous", tr("Piste précédente")),
                ("stop", tr("Arrêter")),
                ("mute", tr("Muet")),
                ("volume_up", "Volume +"),
                ("volume_down", "Volume −"),
            ];
            let label = items
                .iter()
                .find(|(id, _)| *id == command)
                .map(|(_, label)| *label)
                .unwrap_or(tr("Choisir"));
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
        Step::TextOp { name, op, value, find, persistent } => {
            ui.label(tr("Variable texte"));
            changed |= ui.text_edit_singleline(name).changed();
            let choices = [("set", tr("Définir")), ("append", tr("Ajouter à la fin")), ("prepend", tr("Ajouter au début")),
                ("upper", tr("Mettre en majuscules")), ("lower", tr("Mettre en minuscules")), ("trim", tr("Supprimer les espaces")),
                ("replace", tr("Remplacer")), ("length", tr("Compter les caractères"))];
            let selected = choices.iter().find(|(id, _)| *id == op).map(|(_, label)| *label).unwrap_or(tr("Opération inconnue"));
            egui::ComboBox::from_id_salt("text_op_choice").selected_text(selected).show_ui(ui, |ui| {
                for (id, label) in choices { changed |= ui.selectable_value(op, id.into(), label).changed(); }
            });
            if ["set", "append", "prepend", "replace"].contains(&op.as_str()) {
                ui.label(tr("Valeur (accepte {variable})")); changed |= ui.text_edit_multiline(value).changed();
            }
            egui::CollapsingHeader::new(tr("Options avancées")).id_salt("text_op_advanced").show(ui, |ui| {
                if op == "replace" { ui.label(tr("Texte recherché (accepte {variable}, non vide après interpolation)")); changed |= ui.text_edit_singleline(find).changed(); }
                else { ui.label(tr("Remplacer : sélectionner l’opération pour préciser le texte recherché.")); }
                ui.label(tr("Longueur : nombre de caractères Unicode ; le résultat est stocké dans la même variable."));
                changed |= ui.checkbox(persistent, tr("Conserver entre les macros")).changed();
            });
        }
        Step::Notify { title, message } => {
            ui.label(tr("Titre")); changed |= ui.text_edit_singleline(title).changed();
            ui.label(tr("Message (variables : {nom} ; {{ pour une accolade)")); changed |= ui.text_edit_multiline(message).changed();
        }
        Step::WaitUntil { condition, timeout_ms, poll_ms } => {
            changed |= edit_condition(ui, condition, audio, 0);
            egui::CollapsingHeader::new(tr("Options avancées")).id_salt("wait_until_advanced").show(ui, |ui| {
                ui.label(tr("Délai maximal (ms)"));
                changed |= ui.add(egui::DragValue::new(timeout_ms).range(50..=600_000)).changed();
                ui.label(tr("Intervalle de vérification (ms)"));
                changed |= ui.add(egui::DragValue::new(poll_ms).range(50..=600_000)).changed();
            });
        }
        Step::IfCondition { condition, .. } => {
            changed |= edit_condition(ui, condition, audio, 0);
            ui.label(tr("Configure ALORS et SINON dans l’Atelier blocs."));
        }
        Step::IfWindow { title, .. } => {
            ui.label(tr("Le titre de la fenêtre contient"));
            changed |= ui.text_edit_singleline(title).changed();
            ui.label(tr("Configure ALORS et SINON dans l’Atelier blocs."));
        }
        Step::Repeat { count, .. } => {
            ui.horizontal(|ui| {
                ui.label(tr("Répéter"));
                changed |= ui.add(egui::DragValue::new(count).range(1..=100)).changed();
                ui.label(tr("fois les blocs de l’Atelier"));
            });
        }
        Step::Math { name, op, value, min, max, persistent } => {
            ui.label(tr("Variable numérique")); changed |= ui.text_edit_singleline(name).changed();
            ui.label(tr("Opération"));
            egui::ComboBox::from_id_salt("math_op").selected_text(match op.as_str() {
                "set" => tr("Définir"), "add" => tr("Ajouter"), "subtract" => tr("Soustraire"),
                "multiply" => tr("Multiplier"), "divide" => tr("Diviser"), "modulo" => tr("Modulo"),
                "min" => tr("Minimum"), "max" => tr("Maximum"), _ => tr("Aléatoire"),
            }).show_ui(ui, |ui| {
                for (id, label) in [("set", tr("Définir")), ("add", tr("Ajouter")), ("subtract", tr("Soustraire")),
                    ("multiply", tr("Multiplier")), ("divide", tr("Diviser")), ("modulo", tr("Modulo")),
                    ("min", "Minimum"), ("max", "Maximum"), ("random", tr("Aléatoire"))] {
                    changed |= ui.selectable_value(op, id.into(), label).changed();
                }
            });
            if op != "random" { ui.label(tr("Valeur (nombre ou {variable})")); changed |= ui.text_edit_singleline(value).changed(); }
            egui::CollapsingHeader::new(tr("Options avancées")).id_salt("math_advanced").show(ui, |ui| {
                if op == "random" {
                    ui.label(tr("Minimum")); changed |= ui.add(egui::DragValue::new(min)).changed();
                    ui.label(tr("Maximum")); changed |= ui.add(egui::DragValue::new(max)).changed();
                }
                changed |= ui.checkbox(persistent, tr("Conserver entre les macros")).changed();
            });
        }
        Step::SetVariable { name, value, persistent } => {
            ui.label(tr("Variable")); changed |= ui.text_edit_singleline(name).changed();
            ui.label(tr("Valeur (accepte {variable})")); changed |= ui.text_edit_singleline(value).changed();
            egui::CollapsingHeader::new(tr("Options avancées")).id_salt("set_variable_advanced").show(ui, |ui| {
                changed |= ui.checkbox(persistent, tr("Conserver entre les macros")).changed();
            });
        }
        Step::IfVariable { name, value, .. } => {
            ui.horizontal(|ui| {
                ui.label(tr("Variable"));
                changed |= ui.text_edit_singleline(name).changed();
            });
            ui.horizontal(|ui| {
                ui.label(tr("Valeur"));
                changed |= ui.text_edit_singleline(value).changed();
            });
        }
        Step::AutoHotkey {
            executable,
            script,
            args,
        } => {
            ui.horizontal(|ui| {
                ui.label(tr("AutoHotkey v2"));
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
            ui.label(tr("Arguments : un par ligne"));
            if ui.text_edit_multiline(&mut text).changed() {
                *args = text.lines().map(String::from).collect();
                changed = true;
            }
            ui.label(RichText::new(tr("Le script agit sur ton PC lors de l’exécution. Arrêter la macro termine le processus AutoHotkey lancé par NovyDeck.")).small());
        }
        Step::Back | Step::PageNext | Step::PagePrevious => {
            ui.label(tr("Navigation dans ce profil, aussi disponible depuis les touches du périphérique."));
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

fn mouse_origin_editor(ui: &mut egui::Ui, origin: &mut String, changed: &mut bool) {
    egui::ComboBox::from_id_salt("mouse_origin").selected_text(match origin.as_str() { "window" => tr("Fenêtre (zone client)"), "cursor" => tr("Curseur"), _ => tr("Écran") }).show_ui(ui, |ui| {
        for (id, label) in [("screen", tr("Écran")), ("window", tr("Fenêtre (zone client)")), ("cursor", tr("Curseur"))] { *changed |= ui.selectable_value(origin, id.into(), label).changed(); }
    });
}
fn step_summary(step: &Step) -> String {
    match step {
        Step::Hotkey { keys } => keys.clone(),
        Step::KeyHold { .. } => step.key_hold_summary().unwrap_or_default(),
        Step::Delay { ms } => trf("Attendre {ms} ms", &[("ms", &ms)]),
        Step::MouseClick { button, double, x, y, relative_to } => trf("Clic {button}{double}{position}", &[("button", &button), ("double", &if *double { tr(" double") } else { "" }), ("position", &x.zip(*y).map(|(x, y)| trf(" à {x}, {y} ({relative_to})", &[("x", &x), ("y", &y), ("relative_to", &relative_to)])).unwrap_or_default())]),
        Step::MouseMove { x, y, relative, relative_to } => trf("Souris {x}, {y} ({origin})", &[("x", &x), ("y", &y), ("origin", &if *relative { "cursor" } else { relative_to })]),
        Step::MouseScroll { amount, horizontal } => trf("Défiler de {amount} crans {direction}", &[("amount", &amount), ("direction", &if *horizontal { tr("horizontalement") } else { tr("verticalement") })]),
        Step::PlaySound { path, volume } => trf("Son : {path} ({volume} %)", &[("path", &path), ("volume", &volume)]),
        Step::IfCondition { condition, .. } => trf("Si {condition}", &[("condition", &condition.summary())]),
        Step::WaitUntil { condition, timeout_ms, .. } => trf("Attendre que {condition} ({timeout} ms max)", &[("condition", &condition.summary()), ("timeout", &timeout_ms)]),
        Step::Notify { title, message } => trf("Notification « {title} » : {message}", &[("title", &title), ("message", &message.chars().take(50).collect::<String>())]),
        Step::Math { .. } => step.math_summary().unwrap_or_default(),
        Step::TextOp { .. } => step.text_op_summary().unwrap_or_default(),
        Step::SetVariable { name, persistent, .. } => trf("Définir {name}{persistent}", &[("name", &name), ("persistent", &if *persistent { tr(" (enregistrée)") } else { "" })]),
        Step::Clipboard { .. } => step.clipboard_summary().unwrap_or_default(),
        Step::WindowOp { .. } => step.window_op_summary().unwrap_or_default(),
        Step::AppVolume { .. } => step.app_volume_summary().unwrap_or_default(),
        Step::DeviceVolume { .. } => step.device_volume_summary().unwrap_or_default(),
        Step::StopSounds => tr("Arrêter les sons").into(),
        Step::Toggle { a, b } => trf("Basculer 1 / 2 ({a} / {b} blocs)", &[("a", &a.len()), ("b", &b.len())]),
        Step::UdpSend { host, port, .. } => format!("UDP : {host}:{port}"),
        Step::HttpRequest { method, url, .. } => format!("HTTP {method} : {url}"),
        Step::D6Brightness { mode, value } => trf("Luminosité : {mode} {value} %", &[("mode", &mode), ("value", &value)]),
        Step::D6Sleep => tr("Veille du périphérique").into(),
        Step::CloseWindow => tr("Fermer la fenêtre active").into(),
        Step::Media { command } => trf("Média : {command}", &[("command", &command)]),
        Step::Url { url } => url.clone(),
        Step::Text { .. } => tr("Texte").into(),
        Step::Launch { program, .. } => trf("Lancer {program}", &[("program", &program)]),
        Step::WaitWindow { title, .. } => trf("Attendre {title}", &[("title", &title)]),
        Step::Profile { name } => trf("Profil {name}", &[("name", &name)]),
        Step::Folder { id } => trf("Dossier {id}", &[("id", &id)]),
        Step::Page { id } => trf("Page {id}", &[("id", &id)]),
        Step::Back => tr("Retour").into(),
        Step::PageNext => tr("Page suivante").into(),
        Step::PagePrevious => tr("Page précédente").into(),
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
        egui::ComboBox::from_id_salt(ui.next_auto_id()).selected_text(tr("Choisir…")).show_ui(ui, |ui| {
            for item in options {
                if ui.selectable_label(value == item, item).clicked() { *value = item.clone(); changed = true; }
            }
        });
    }
    changed
}
