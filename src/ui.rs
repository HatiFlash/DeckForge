use crate::{
    control::{Decision, InputGate},
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
    wanted: bool,
    status: String,
    logs: VecDeque<String>,
    started: Instant,
    last_edit: Instant,
    new_step: usize,
    show_logs: bool,
    settings: bool,
    rename: bool,
    notice: String,
    error: bool,
    smoke: bool,
    frames: usize,
    capturing: bool,
    last_key: Option<(usize, Instant)>,
    textures: HashMap<String, Option<egui::TextureHandle>>,
    assistant: Option<crate::mcp_server::Server>,
    proposals: crate::assistant::Proposals,
    assistant_cache: HashMap<String, Result<serde_json::Value, String>>,
    assistant_settings: bool,
    http_child: Option<std::process::Child>,
}
impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        let path = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("DeckForge.exe"))
            .with_file_name("config.json");
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
        let smoke = std::env::args().any(|a| a == "--ui-smoke");
        let (tx, events) = mpsc::channel();
        let device = device::spawn(tx.clone());
        let engine = Engine::new(tx);
        let mut gate = InputGate::default();
        gate.invalid = load_error.is_some();
        let assistant =
            crate::mcp_server::Server::new(path.with_file_name("mcp-connection.json")).ok();
        let mut app = Self {
            applied: config.clone(),
            config,
            pending: None,
            path,
            selected: 0,
            engine,
            device,
            events,
            gate,
            wanted: false,
            status: "D6 non connecté".into(),
            logs: VecDeque::new(),
            started: Instant::now(),
            last_edit: Instant::now(),
            new_step: 0,
            show_logs: false,
            settings: false,
            rename: false,
            notice: "Tout est prêt.".into(),
            error: false,
            smoke,
            frames: 0,
            capturing: false,
            last_key: None,
            textures: HashMap::new(),
            assistant,
            proposals: Default::default(),
            assistant_cache: HashMap::new(),
            assistant_settings: false,
            http_child: None,
        };
        if let Some(e) = load_error {
            app.fail(e);
        } else if !smoke || std::env::args().any(|a| a == "--connect") {
            app.connect();
        }
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
        self.gate.dirty = true;
        self.last_edit = Instant::now();
    }
    fn upload(&mut self, connect: bool) {
        let revision = self.gate.request_apply();
        self.pending = Some((revision, self.config.clone()));
        let command = if connect {
            Command::Connect(self.config.clone(), revision)
        } else {
            Command::Update(self.config.clone(), revision)
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
        if let Err(e) = self.config.validate() {
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
                if self.wanted {
                    self.upload(false);
                }
                self.log("Modifications enregistrées.".into());
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
            self.log("Cette touche n’a pas encore d’action. Ajoute une étape à droite.".into());
            return;
        }
        self.engine.start(key, dry, if delayed { 3000 } else { 0 });
    }
    fn preview(&mut self, dry: bool) {
        let key = self.config.profiles[self.config.active].keys[self.selected].clone();
        let c = self.config.clone();
        self.execute_key(key, dry, !dry, &c);
    }
    fn events(&mut self) {
        for _ in 0..1000 {
            let Ok(event) = self.events.try_recv() else {
                break;
            };
            match event {
                Event::Log(s) => self.log(s),
                Event::Device(connected, message) => {
                    if self.status != message {
                        self.log(message.clone());
                    }
                    self.status = message;
                    if !connected {
                        self.gate.disconnect();
                        self.engine.stop();
                    } else {
                        self.gate.connected = true;
                    }
                }
                Event::Applied(revision) => {
                    if self.gate.applied(revision) {
                        if let Some((r, c)) = self.pending.take() {
                            if r == revision {
                                self.applied = c;
                            }
                        }
                        self.log(
                            "D6 synchronisé. Les touches suivent le mode affiché en haut.".into(),
                        );
                    }
                }
                Event::Key(index, at) => {
                    self.last_key = Some((index, Instant::now()));
                    match self.gate.decision_for_age(at.elapsed()) {
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
                            let c = self.applied.clone();
                            let key = c.profiles[c.active].keys[index].clone();
                            self.execute_key(key, false, false, &c);
                        }
                    }
                }
                Event::Profile(name) => {
                    if let Some(index) = self.config.profiles.iter().position(|p| p.name == name) {
                        self.config.active = index;
                        self.save();
                    }
                }
            }
        }
    }
    fn select_profile(&mut self, index: usize) {
        if index != self.config.active {
            self.engine.stop();
            self.config.active = index;
            self.selected = 0;
            self.save();
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
        self.selected = 0;
        self.edited();
    }
    fn import(&mut self) {
        self.gate.enabled = false;
        self.engine.stop();
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Configuration DeckForge", &["json", "bak"])
            .pick_file()
        {
            match store::load(&path) {
                Ok(c) => {
                    self.config = c;
                    self.selected = 0;
                    self.gate.invalid = false;
                    self.edited();
                    self.log(
                        "Profils importés. Les touches restent en pause jusqu’à leur réactivation."
                            .into(),
                    );
                }
                Err(e) => self.fail(format!("Import impossible : {e}")),
            }
        }
    }
    fn export(&mut self) {
        self.gate.enabled = false;
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("deckforge-profils.json")
            .add_filter("JSON", &["json"])
            .save_file()
        {
            match store::save(&path, &self.config) {
                Ok(()) => {
                    self.log("Profils exportés. Réactive les touches quand tu es prêt.".into())
                }
                Err(e) => self.fail(format!("Export impossible : {e}")),
            }
        }
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
            "deckforge_get_state" => Ok(
                json!({"connected":self.gate.connected,"physical_keys":format!("{:?}",self.gate.decision()),"current_profile":self.config.profiles[self.config.active].name,"profiles":self.config.profiles.iter().map(|p|json!({"name":p.name,"keys":p.keys.iter().enumerate().map(|(i,k)|json!({"key":i+1,"label":k.label,"steps":k.steps.len()})).collect::<Vec<_>>()})).collect::<Vec<_>>(),"pending_proposals":self.proposals.pending.len(),"configuration_invalid":self.gate.invalid}),
            ),
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
            "deckforge_create_profile" | "deckforge_configure_key" | "deckforge_request_run" => {
                if self.gate.invalid {
                    return Err("Récupère d’abord la configuration dans DeckForge".into());
                }
                self.proposals.propose(name, args, &self.config)
            }
            "deckforge_simulate_macro" => {
                let (p, k) = crate::assistant::position(&self.config, args)?;
                let c = self.config.clone();
                let key = c.profiles[p].keys[k].clone();
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
    fn assistant_window(&mut self, ctx: &egui::Context) {
        if self.assistant_settings {
            let mut open = true;
            egui::Window::new("Assistant IA").open(&mut open).collapsible(false).resizable(false).default_width(460.0).show(ctx,|ui|{
            ui.label(RichText::new("Prépare tes macros avec ton IA.").size(20.0).strong());ui.add_space(10.0);
            let mut enabled=self.assistant.as_ref().is_some_and(|s|s.enabled());
            ui.horizontal(|ui|{if theme::toggle(ui,&mut enabled).changed(){if let Some(server)=&self.assistant{server.set_enabled(enabled);}if !enabled{self.stop_http();self.proposals.reject_all();self.assistant_cache.clear();}self.log(if enabled{"Connecteur MCP activé."}else{"Connecteur MCP désactivé."}.into());}ui.label(RichText::new(if enabled{"Assistant IA activé"}else{"Assistant IA désactivé"}).strong());});
            ui.add_space(10.0);ui.label("Ton assistant peut lire les profils et proposer des actions. Les changements et les exécutions réelles attendent toujours ta validation ici.");ui.add_space(8.0);
            if self.assistant.is_none(){ui.colored_label(Color32::DARK_RED,"Le connecteur local n’a pas pu démarrer.");}
            ui.label(RichText::new("Connexion locale MCP · Claude, Codex et clients compatibles stdio. Aucune clé d’IA à saisir dans DeckForge.").small().color(MUTED));
            if ui.button("Copier la configuration MCP").clicked(){let exe=self.path.with_file_name("deckforge-mcp.exe");let config=serde_json::json!({"mcpServers":{"deckforge":{"command":exe.to_string_lossy(),"args":[]}}});ui.ctx().copy_text(serde_json::to_string_pretty(&config).unwrap());self.log("Configuration MCP copiée.".into());}
            ui.label(RichText::new("Le guide MCP fourni avec l’application détaille la connexion à ChatGPT et aux autres clients.").small().color(MUTED));
            ui.add_space(14.0);ui.separator();ui.label(RichText::new("ChatGPT web / clients HTTP").strong());
            ui.label(RichText::new("Le mode HTTP reste local au PC. Un tunnel ou un proxy compatible est nécessaire pour un service IA distant.").small().color(MUTED));
            ui.horizontal(|ui|{if ui.add_enabled(enabled,egui::Button::new(if self.http_child.is_some(){"Arrêter HTTP"}else{"Démarrer HTTP"})).clicked(){if self.http_child.is_some(){self.stop_http();}else{self.start_http();}}if self.http_child.is_some()&&ui.button("Copier l’adresse").clicked(){ui.ctx().copy_text("http://127.0.0.1:9846/mcp".into());}});
            if self.http_child.is_some()&&ui.button("Copier la clé HTTP").clicked(){match std::fs::read_to_string(self.path.with_file_name("mcp-http-token.txt")){Ok(t)=>{ui.ctx().copy_text(t.trim().into());self.log("Clé HTTP copiée. Utilise-la uniquement dans ton client MCP.".into());},Err(_)=>self.fail("La clé n’est pas encore prête. Réessaie dans un instant.".into())}}
        });
            self.assistant_settings = open;
        }
        let mut decision = None;
        if let Some(p) = self.proposals.pending.front() {
            egui::Window::new("Proposition de ton assistant").id(egui::Id::new("ai_review")).collapsible(false).resizable(true).default_width(570.0).show(ctx,|ui|{
                theme::pill(ui,"VALIDATION NÉCESSAIRE",BLUE);ui.add_space(8.0);ui.label(RichText::new(&p.title).size(18.0).strong());
                ui.label(RichText::new("Aucun changement ni lancement n’a encore eu lieu. Vérifie les actions avant de continuer.").color(MUTED));
                egui::ScrollArea::vertical().max_height(360.0).show(ui,|ui|{ui.label(&p.details);});ui.add_space(12.0);
                ui.horizontal(|ui|{if ui.button("Refuser").clicked(){decision=Some(false);}if ui.add(theme::primary(RichText::new("Valider cette proposition").color(WHITE))).clicked(){decision=Some(true);}});
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
            let report = serde_json::json!({"rendered":true,"connected":self.gate.connected,"physical_keys":format!("{:?}",self.gate.decision()),"version":"0.2.0","image_size":img.size,"assistant_enabled":self.assistant.as_ref().is_some_and(|s|s.enabled())});
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
    fn header(&mut self, ctx: &egui::Context) {
        let running = self.engine.running.load(Ordering::SeqCst);
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(WHITE)
                    .inner_margin(egui::Margin::symmetric(24, 16)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(34.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 10.0, BLUE);
                    for y in 0..2 {
                        for x in 0..3 {
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    r.min + Vec2::new(7.0 + x as f32 * 7.0, 10.0 + y as f32 * 8.0),
                                    Vec2::splat(4.0),
                                ),
                                1.0,
                                WHITE,
                            );
                        }
                    }
                    ui.vertical(|ui| {
                        ui.label(RichText::new("DeckForge").size(21.0).strong());
                        ui.label(
                            RichText::new("Ton espace de raccourcis")
                                .small()
                                .color(MUTED),
                        );
                    });
                    ui.add_space(18.0);
                    theme::pill(
                        ui,
                        if self.gate.connected {
                            "D6 connecté"
                        } else if self.wanted {
                            "Connexion…"
                        } else {
                            "D6 déconnecté"
                        },
                        if self.gate.connected { GREEN } else { MUTED },
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(
                                RichText::new(if running {
                                    "Arrêter la macro"
                                } else {
                                    "Tout arrêter"
                                })
                                .color(Color32::from_rgb(206, 58, 64)),
                            )
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
                            self.log(
                                if self.gate.enabled {
                                    "Touches réactivées."
                                } else {
                                    "Touches en pause."
                                }
                                .into(),
                            );
                        }
                        let active = matches!(self.gate.decision(), Decision::Execute);
                        ui.label(
                            RichText::new(if active {
                                "Touches actives"
                            } else if self.gate.inspect {
                                "Diagnostic"
                            } else if !self.gate.enabled {
                                "En pause"
                            } else {
                                "À synchroniser"
                            })
                            .strong()
                            .color(if active { BLUE } else { MUTED }),
                        );
                        if self.gate.dirty
                            && ui
                                .add(theme::primary(RichText::new("Enregistrer").color(WHITE)))
                                .clicked()
                        {
                            self.save();
                        }
                    });
                });
            });
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sidebar")
            .exact_width(184.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(BG).inner_margin(16))
            .show(ctx, |ui| {
                ui.add_space(15.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("PROFILS").size(10.0).strong().color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                self.config.profiles.len() < 64,
                                egui::Button::new("+").frame(false),
                            )
                            .on_hover_text("Créer un profil")
                            .clicked()
                        {
                            self.add_profile(false);
                        }
                    });
                });
                let mut choose = None;
                for (i, p) in self.config.profiles.iter().enumerate() {
                    let selected = i == self.config.active;
                    let text = RichText::new(&p.name)
                        .color(if selected { BLUE } else { INK })
                        .strong();
                    if ui
                        .add_sized(
                            [ui.available_width(), 36.0],
                            egui::Button::new(text)
                                .fill(if selected { WASH } else { BG })
                                .stroke(egui::Stroke::NONE)
                                .corner_radius(10),
                        )
                        .clicked()
                    {
                        choose = Some(i);
                    }
                }
                if let Some(i) = choose {
                    self.select_profile(i);
                }
                ui.add_space(24.0);
                ui.label(
                    RichText::new("BIBLIOTHÈQUE")
                        .size(10.0)
                        .strong()
                        .color(MUTED),
                );
                if ui
                    .add_sized(
                        [ui.available_width(), 34.0],
                        egui::Button::new("Réglages du D6").frame(false),
                    )
                    .clicked()
                {
                    self.settings = true;
                }
                if ui
                    .add_sized(
                        [ui.available_width(), 34.0],
                        egui::Button::new("Importer des profils").frame(false),
                    )
                    .clicked()
                {
                    self.import();
                }
                if ui
                    .add_sized(
                        [ui.available_width(), 34.0],
                        egui::Button::new("Exporter mes profils").frame(false),
                    )
                    .clicked()
                {
                    self.export();
                }
                if ui
                    .add_sized(
                        [ui.available_width(), 34.0],
                        egui::Button::new("Journal d’activité").frame(false),
                    )
                    .clicked()
                {
                    self.show_logs = !self.show_logs;
                }
                let ai_on = self.assistant.as_ref().is_some_and(|s| s.enabled());
                if ui
                    .add_sized(
                        [ui.available_width(), 34.0],
                        egui::Button::new(
                            RichText::new(if ai_on {
                                "Assistant IA · actif"
                            } else {
                                "Assistant IA"
                            })
                            .color(if ai_on { BLUE } else { INK }),
                        )
                        .frame(false),
                    )
                    .clicked()
                {
                    self.assistant_settings = true;
                }
                ui.add_space(24.0);
                theme::card().inner_margin(12).show(ui, |ui| {
                    ui.label(RichText::new("FIFINE D6").strong());
                    ui.label(RichText::new("15 touches LCD").small().color(MUTED));
                    ui.add_space(8.0);
                    let (r, _) =
                        ui.allocate_exact_size(Vec2::new(125.0, 63.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(r, 8.0, Color32::from_rgb(34, 37, 45));
                    for y in 0..3 {
                        for x in 0..5 {
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    r.min + Vec2::new(8.0 + x as f32 * 23.0, 7.0 + y as f32 * 18.0),
                                    Vec2::new(17.0, 13.0),
                                ),
                                3.0,
                                if self.gate.connected {
                                    BLUE
                                } else {
                                    Color32::from_gray(90)
                                },
                            );
                        }
                    }
                    ui.add_space(5.0);
                    if ui
                        .add_sized(
                            [ui.available_width(), 30.0],
                            egui::Button::new(if self.wanted {
                                if self.gate.connected {
                                    "Déconnecter"
                                } else {
                                    "Annuler"
                                }
                            } else {
                                "Connecter"
                            }),
                        )
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
                    }
                });
                ui.add_space(20.0);
                ui.label(
                    RichText::new("DeckForge 0.2\nConçu pour Windows")
                        .small()
                        .color(MUTED),
                );
            });
    }
    fn footer(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::new()
                    .fill(WHITE)
                    .inner_margin(egui::Margin::symmetric(20, 7)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if self.gate.dirty {
                            "Sauvegarde automatique…"
                        } else {
                            &self.notice
                        })
                        .size(11.0)
                        .color(if self.error {
                            Color32::DARK_RED
                        } else {
                            MUTED
                        }),
                    );
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
                                .color(MUTED),
                        );
                    });
                });
            });
        if self.show_logs {
            egui::TopBottomPanel::bottom("logs")
                .default_height(165.0)
                .height_range(100.0..=320.0)
                .resizable(true)
                .frame(egui::Frame::new().fill(WHITE).inner_margin(16))
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
                                ui.label(RichText::new(s).monospace().size(11.0).color(MUTED));
                            }
                        });
                });
        }
    }
    fn editor(&mut self, ctx: &egui::Context) {
        let running = self.engine.running.load(Ordering::SeqCst);
        egui::SidePanel::right("editor").exact_width(360.0).resizable(false).frame(egui::Frame::new().fill(WHITE).inner_margin(20)).show(ctx,|ui|{
            ui.add_space(6.0);ui.horizontal(|ui|{ui.label(RichText::new(format!("Touche {:02}",self.selected+1)).size(22.0).strong());ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{theme::pill(ui,"ÉDITEUR",BLUE);});});
            ui.label(RichText::new("Personnalise son apparence et ses actions.").small().color(MUTED));ui.add_space(15.0);
            let names=self.config.profiles.iter().map(|p|p.name.clone()).collect::<Vec<_>>();let active=self.config.active;
            let key=&mut self.config.profiles[active].keys[self.selected];let mut changed=false;
            ui.label(RichText::new("Nom de la touche").small().strong());changed|=ui.add(egui::TextEdit::singleline(&mut key.label).desired_width(f32::INFINITY).hint_text("Ex. Lecture, Mon application…").margin(Vec2::new(10.0,8.0))).changed();
            ui.horizontal(|ui|{ui.label(RichText::new("Couleur").small().color(MUTED));changed|=ui.color_edit_button_srgb(&mut key.color).changed();ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.small_button(if key.image.is_empty(){"Ajouter une image"}else{"Changer l’image"}).clicked(){if let Some(path)=rfd::FileDialog::new().add_filter("Image",&["png","jpg","jpeg"]).pick_file(){key.image=path.to_string_lossy().into_owned();changed=true;}}});});
            if !key.image.is_empty(){ui.horizontal(|ui|{ui.label(RichText::new(PathBuf::from(&key.image).file_name().unwrap_or_default().to_string_lossy()).small().color(MUTED));if ui.small_button("Retirer").clicked(){key.image.clear();changed=true;}});}
            ui.add_space(12.0);ui.horizontal(|ui|{ui.label(RichText::new("Actions").size(17.0).strong());ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{ui.label(RichText::new(format!("{} étape(s)",key.steps.len())).small().color(MUTED));});});
            ui.horizontal(|ui|{ui.label(RichText::new("Répéter").small().color(MUTED));changed|=ui.add(egui::DragValue::new(&mut key.repeat).range(1..=1000)).changed();ui.label(RichText::new("fois").small().color(MUTED));});
            let mut remove=None;let mut movement=None;
            egui::ScrollArea::vertical().id_salt("step_list").max_height((ui.available_height()-160.0).max(120.0)).show(ui,|ui|{
                if key.steps.is_empty(){theme::card().fill(BG).show(ui,|ui|{ui.label(RichText::new("Une touche, plusieurs possibilités.").strong());ui.label(RichText::new("Ajoute une action ci-dessous pour commencer.").color(MUTED));});}
                let count=key.steps.len();
                for (i,step)in key.steps.iter_mut().enumerate(){ui.push_id(i,|ui|{egui::Frame::new().fill(BG).corner_radius(12).inner_margin(12).show(ui,|ui|{
                    ui.horizontal(|ui|{ui.label(RichText::new(format!("{:02}",i+1)).size(11.0).color(BLUE).strong());ui.label(RichText::new(step.name()).size(13.0).strong());ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.small_button("×").on_hover_text("Retirer l’étape").clicked(){remove=Some(i);}if ui.add_enabled(i+1<count,egui::Button::new("↓").small()).clicked(){movement=Some((i,i+1));}if ui.add_enabled(i>0,egui::Button::new("↑").small()).clicked(){movement=Some((i,i-1));}});});
                    changed|=edit_step(ui,step,&names);
                });ui.add_space(3.0);});}
            });
            if let Some(i)=remove{key.steps.remove(i);changed=true;}if let Some((a,b))=movement{key.steps.swap(a,b);changed=true;}
            ui.add_space(8.0);
            let actions=["Pause","Raccourci clavier","Saisir du texte","Application","Site web","Attendre une fenêtre","Média / volume","Changer de profil"];
            ui.horizontal(|ui|{egui::ComboBox::from_id_salt("add_action").width(195.0).selected_text(actions[self.new_step]).show_ui(ui,|ui|{for (i,name)in actions.iter().enumerate(){ui.selectable_value(&mut self.new_step,i,*name);}});if ui.add_enabled(key.steps.len()<256,egui::Button::new("+").min_size(Vec2::splat(32.0))).clicked(){key.steps.push(match self.new_step{0=>Step::Delay{ms:500},1=>Step::Hotkey{keys:"CTRL+C".into()},2=>Step::Text{text:String::new()},3=>Step::Launch{program:String::new(),args:vec![]},4=>Step::Url{url:"https://".into()},5=>Step::WaitWindow{title:String::new(),timeout_ms:10000,focus:true},6=>Step::Media{command:"play_pause".into()},_=>Step::Profile{name:names[active].clone()}});changed=true;}});
            ui.add_space(9.0);
            ui.horizontal(|ui|{if ui.add_enabled(!running,egui::Button::new("Simuler").min_size(Vec2::new(95.0,36.0))).clicked(){self.preview(true);}if ui.add_enabled(!running,theme::primary(RichText::new("Exécuter dans 3 s").color(WHITE)).min_size(Vec2::new(175.0,36.0))).clicked(){self.preview(false);}});
            ui.label(RichText::new("Simuler n’a aucun effet sur ton PC. Le test réel te laisse 3 secondes pour changer de fenêtre.").size(10.0).color(MUTED));
            if changed{self.edited();}
        });
    }
    fn deck(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(26)).show(ctx,|ui|{egui::ScrollArea::vertical().show(ui,|ui|{
            ui.horizontal(|ui|{ui.vertical(|ui|{ui.heading(&self.config.profiles[self.config.active].name);ui.label(RichText::new("Choisis une touche. Crée ton prochain raccourci.").color(MUTED).size(13.0));});ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.button("•••").clicked(){self.rename=true;}});});
            ui.add_space(22.0);
            let width=((ui.available_width()-48.0)/5.0).clamp(62.0,145.0);let height=if width<88.0{112.0}else{126.0};
            egui::Grid::new("deck_grid_v2").spacing([12.0,12.0]).show(ui,|ui|{
                for i in 0..15{let key=&self.config.profiles[self.config.active].keys[i];let selected=i==self.selected;let flashing=self.last_key.is_some_and(|(idx,t)|idx==i&&t.elapsed()<Duration::from_millis(350));
                    let (r,response)=ui.allocate_exact_size(Vec2::new(width,height),egui::Sense::click());
                    let shadow=r.translate(Vec2::new(0.0,3.0));ui.painter().rect_filled(shadow,16.0,Color32::from_black_alpha(5));
                    ui.painter().rect_filled(r,16.0,if selected{Color32::from_rgb(245,250,255)}else{WHITE});
                    ui.painter().rect_stroke(r,16.0,egui::Stroke::new(if selected||flashing{2.0_f32}else{1.0_f32},if selected||flashing{BLUE}else{LINE}),egui::StrokeKind::Inside);
                    let color=Color32::from_rgb(key.color[0],key.color[1],key.color[2]);let center=r.center_top()+Vec2::new(0.0,34.0);
                    let mut has_image=false;
                    if !key.image.is_empty(){let texture=self.textures.entry(key.image.clone()).or_insert_with(||{
                        let mut reader=image::ImageReader::open(&key.image).ok()?.with_guessed_format().ok()?;let mut limits=image::Limits::default();limits.max_image_width=Some(8192);limits.max_image_height=Some(8192);limits.max_alloc=Some(64*1024*1024);reader.limits(limits);
                        let img=reader.decode().ok()?.thumbnail(112,112).into_rgba8();let size=[img.width()as usize,img.height()as usize];Some(ctx.load_texture(key.image.clone(),egui::ColorImage::from_rgba_unmultiplied(size,&img),egui::TextureOptions::LINEAR))});
                        if let Some(texture)=texture{ui.painter().image(texture.id(),egui::Rect::from_center_size(center,Vec2::splat(36.0)),egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.0,1.0)),WHITE);has_image=true;}
                    }
                    if !has_image{ui.painter().circle_filled(center,20.0,color.gamma_multiply(0.09));theme::symbol(ui.painter(),center,19.0,key.steps.first().map(step_kind).unwrap_or(8),color);}
                    let mut job=egui::text::LayoutJob::default();job.wrap.max_width=width-16.0;job.wrap.max_rows=2;job.wrap.break_anywhere=true;job.halign=egui::Align::Center;
                    job.append(if key.label.is_empty(){"Ajouter"}else{&key.label},0.0,egui::TextFormat{font_id:egui::FontId::proportional(if width<88.0{11.0}else{13.0}),color:if key.label.is_empty(){MUTED}else{INK},..Default::default()});
                    let galley=ui.painter().layout_job(job);let pos=egui::pos2(r.center().x,r.top()+62.0);ui.painter().galley(pos,galley,INK);
                    ui.painter().text(r.center_bottom()-Vec2::new(0.0,12.0),egui::Align2::CENTER_BOTTOM,format!("{:02}",i+1),egui::FontId::proportional(9.0),MUTED);
                    if response.on_hover_text(format!("Touche {:02} · {}",i+1,key.label)).clicked(){self.selected=i;}
                    if i%5==4{ui.end_row();}
                }
            });
            ui.add_space(20.0);
            let (title,description,color)=match self.gate.decision(){Decision::Execute=>("Prêt à jouer","Les appuis sur le D6 exécutent les actions enregistrées.",GREEN),Decision::Inspect=>("Mode diagnostic","Un appui sélectionne la touche. Aucune action ne s’exécute.",BLUE),Decision::Blocked(reason)=>{let description=if self.gate.dirty{"Les touches reprendront après la sauvegarde automatique."}else if !self.gate.enabled{"Réactive l’interrupteur en haut pour utiliser les touches."}else if !self.gate.connected{"Connecte le D6. Ferme l’application FIFINE si elle est ouverte."}else{"Les touches reprendront une fois les images envoyées au D6."};(reason,description,MUTED)}};
            theme::card().show(ui,|ui|{ui.horizontal(|ui|{let(r,_)=ui.allocate_exact_size(Vec2::splat(10.0),egui::Sense::hover());ui.painter().circle_filled(r.center(),4.0,color);ui.label(RichText::new(title).strong().color(color));});ui.label(RichText::new(description).size(12.0).color(MUTED));});
            ui.add_space(14.0);ui.label(RichText::new("Les modifications valides sont enregistrées automatiquement.\nLes touches restent disponibles quand cette fenêtre est minimisée.").size(11.0).color(MUTED));
        });});
    }
    fn windows(&mut self, ctx: &egui::Context) {
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
                    ui.add_space(16.0);
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
                            .color(MUTED),
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
                            .color(MUTED),
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
                            for k in &mut p.keys {
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
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.events();
        self.assistant_events();
        self.frames += 1;
        ctx.request_repaint_after(Duration::from_millis(60));
        self.screenshot(ctx);
        self.header(ctx);
        self.sidebar(ctx);
        self.footer(ctx);
        self.editor(ctx);
        self.deck(ctx);
        self.windows(ctx);
        self.assistant_window(ctx);
        if self.gate.dirty
            && !self.gate.invalid
            && self.last_edit.elapsed() > Duration::from_millis(800)
            && self.config.validate().is_ok()
        {
            self.save();
        }
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.stop_http();
        if self.gate.dirty && !self.gate.invalid {
            let _ = store::save(&self.path, &self.config);
        }
        self.engine.stop();
        let _ = self.device.send(Command::Disconnect);
    }
}
fn step_kind(step: &Step) -> usize {
    match step {
        Step::Delay { .. } => 0,
        Step::Hotkey { .. } => 1,
        Step::Text { .. } => 2,
        Step::Launch { .. } => 3,
        Step::Url { .. } => 4,
        Step::WaitWindow { .. } => 5,
        Step::Media { command } => match command.as_str() {
            "volume_up" => 9,
            "volume_down" => 10,
            "mute" => 11,
            "next" => 12,
            "previous" => 13,
            "stop" => 14,
            _ => 6,
        },
        Step::Profile { .. } => 7,
    }
}

fn edit_step(ui: &mut egui::Ui, step: &mut Step, profiles: &[String]) -> bool {
    let mut changed = false;
    match step {
        Step::Delay { ms } => {
            ui.horizontal(|ui| {
                changed |= ui
                    .add(egui::DragValue::new(ms).range(0..=3_600_000).speed(50))
                    .changed();
                ui.label("ms");
            });
        }
        Step::Hotkey { keys } => {
            changed |= ui.text_edit_singleline(keys).changed();
            ui.label(RichText::new("Ex. CTRL+SHIFT+S · ALT+TAB · F13").small());
        }
        Step::Text { text } => {
            changed |= ui
                .add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                )
                .changed();
        }
        Step::Launch { program, args } => {
            ui.horizontal(|ui| {
                changed |= ui.text_edit_singleline(program).changed();
                if ui.small_button("…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Application", &["exe", "com"])
                        .pick_file()
                    {
                        *program = path.to_string_lossy().into_owned();
                        changed = true;
                    }
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
