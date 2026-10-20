//! IA locale facultative pour l'assistant de l'atelier.
//!
//! Le modèle ne sert qu'à *traduire* une demande en blocs DeckForge : la réponse est
//! contrainte par un schéma JSON, filtrée par une liste blanche de blocs, puis montrée en
//! aperçu. Rien n'est exécuté ni inséré sans validation de l'utilisateur.
//!
//! Deux moteurs partagent la même API compatible OpenAI (`/v1/chat/completions`) :
//! - `ollama` : serveur Ollama déjà installé par l'utilisateur (option A) ;
//! - `integre` : `llama-server.exe` livré dans `ai\` et lancé par DeckForge (option B).
//!
//! Seules les adresses de bouclage (127.0.0.1, localhost, ::1) sont acceptées : la demande
//! ne quitte jamais le PC.

use crate::catalog::LocalAi;
use crate::model::Step;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Modèle conseillé : Qwen2.5 1,5B Instruct (Apache 2.0), environ 1 Go en Q4.
pub const OLLAMA_MODEL: &str = "qwen2.5:1.5b";
pub const BUILTIN_MODEL_FILE: &str = "qwen2.5-1.5b-instruct-q4_k_m.gguf";
const MAX_STEPS: usize = 8;

// Le discriminant s'appelle « action » : serde_json trie les clés, et « action » précède toutes les
// autres. La grammaire générée depuis le schéma impose cet ordre ; les exemples (sérialisés par
// serde_json) le respectent, sinon un petit modèle se retrouve enfermé dans un ou deux blocs.
const SYSTEM: &str = r#"Tu convertis une demande (français ou anglais, souvent avec des fautes) en blocs d'automatisation DeckForge pour Windows. Réponds uniquement avec un objet JSON {"steps":[...]}.
Blocs autorisés :
- {"action":"delay","ms":1000} attendre.
- {"action":"hotkey","keys":"CTRL+C"} raccourci clavier, touches en majuscules séparées par +.
- {"action":"text","text":"bonjour"} taper un texte.
- {"action":"url","url":"https://..."} ouvrir un site web nommé par l'utilisateur.
- {"action":"launch","args":[],"program":"notepad.exe"} lancer un programme Windows.
- {"action":"media","command":"play_pause"} play_pause, next, previous, stop, mute, volume_up, volume_down.
- {"action":"window_op","op":"maximize"} fenêtre active : minimize (réduire), maximize (agrandir, plein écran), restore, toggle_always_on_top (toujours au premier plan), close_others (fermer toutes les AUTRES fenêtres), minimize_others (réduire les AUTRES).
- {"action":"close_window"} fermer la fenêtre active (« ferme cette fenêtre »).
- {"action":"notify","message":"..."} afficher une notification ou un rappel.
Raccourcis Windows : verrouiller le PC WIN+L ; afficher le bureau WIN+D ; capture d'écran WIN+SHIFT+S ; gestionnaire des tâches CTRL+SHIFT+ESC ; copier CTRL+C ; coller CTRL+V ; coller sans mise en forme CTRL+SHIFT+V ; couper CTRL+X ; annuler CTRL+Z ; rétablir CTRL+Y ; tout sélectionner CTRL+A ; enregistrer CTRL+S ; rechercher WIN+S ; changer de fenêtre ALT+TAB ; Entrée ENTER ; Échap ESC.
Programmes : bloc-notes notepad.exe ; calculatrice calc.exe ; explorateur de fichiers explorer.exe ; paint mspaint.exe.
Le son et la musique sont des blocs media, jamais des sites web. N'ouvre un site que s'il est demandé. Une demande simple donne un seul bloc : n'ajoute jamais de bloc non demandé. Garde l'ordre de la demande. Si rien ne correspond, réponds {"steps":[]}."#;

fn examples() -> Vec<(&'static str, Value)> {
    vec![
        ("ferme tout sauf la fenetre ou je suis puis attend 2 secondes", json!({"steps":[{"action":"window_op","op":"close_others"},{"action":"delay","ms":2000}]})),
        ("open youtube and pause the music", json!({"steps":[{"action":"url","url":"https://www.youtube.com"},{"action":"media","command":"play_pause"}]})),
        ("copie, attends une demi seconde et colle", json!({"steps":[{"action":"hotkey","keys":"CTRL+C"},{"action":"delay","ms":500},{"action":"hotkey","keys":"CTRL+V"}]})),
        ("lance paint et dis moi c'est parti", json!({"steps":[{"action":"launch","program":"mspaint.exe","args":[]},{"action":"notify","message":"C'est parti"}]})),
        ("mets en pause", json!({"steps":[{"action":"media","command":"play_pause"}]})),
        ("affiche le bureau", json!({"steps":[{"action":"hotkey","keys":"WIN+D"}]})),
    ]
}

/// Schéma JSON contraignant la génération (grammaire côté llama.cpp / Ollama).
pub fn schema() -> Value {
    let obj = |kind: &str, props: Value, required: &[&str]| {
        let mut properties = json!({"action": {"const": kind}});
        for (k, v) in props.as_object().unwrap() {
            properties[k] = v.clone();
        }
        let mut req = vec!["action"];
        req.extend_from_slice(required);
        json!({"type":"object","properties":properties,"required":req,"additionalProperties":false})
    };
    let s = json!({"type":"string","maxLength":300});
    json!({
        "type":"object",
        "properties":{"steps":{"type":"array","maxItems":MAX_STEPS,"items":{"anyOf":[
            obj("delay", json!({"ms":{"type":"integer","minimum":0,"maximum":600000}}), &["ms"]),
            obj("hotkey", json!({"keys":s}), &["keys"]),
            obj("text", json!({"text":{"type":"string","maxLength":2000}}), &["text"]),
            obj("url", json!({"url":s}), &["url"]),
            obj("launch", json!({"program":s,"args":{"type":"array","items":s,"maxItems":8}}), &["program","args"]),
            obj("media", json!({"command":{"enum":["play_pause","next","previous","stop","mute","volume_up","volume_down"]}}), &["command"]),
            obj("window_op", json!({"op":{"enum":["minimize","maximize","restore","toggle_always_on_top","close_others","minimize_others"]}}), &["op"]),
            obj("close_window", json!({}), &[]),
            obj("notify", json!({"message":{"type":"string","minLength":1,"maxLength":500}}), &["message"]),
        ]}}},
        "required":["steps"],
        "additionalProperties":false
    })
}

fn messages(request: &str) -> Value {
    let mut list = vec![json!({"role":"system","content":SYSTEM})];
    for (user, answer) in examples() {
        list.push(json!({"role":"user","content":user}));
        list.push(json!({"role":"assistant","content":answer.to_string()}));
    }
    list.push(json!({"role":"user","content":request}));
    Value::Array(list)
}

/// Convertit la réponse du modèle en blocs sûrs. Tout bloc hors liste blanche ou mal formé
/// fait échouer l'ensemble (pas de proposition partielle trompeuse).
pub fn parse_steps(content: &str) -> Result<Vec<Step>, String> {
    let start = content.find('{').ok_or("réponse sans JSON")?;
    let end = content.rfind('}').ok_or("réponse sans JSON")?;
    let value: Value = serde_json::from_str(&content[start..=end])
        .map_err(|e| format!("JSON invalide : {e}"))?;
    let items = value["steps"].as_array().ok_or("champ steps absent")?;
    if items.len() > MAX_STEPS {
        return Err("trop de blocs proposés".into());
    }
    let mut steps = Vec::new();
    for item in items {
        let kind = item["action"].as_str().or(item["type"].as_str()).unwrap_or_default();
        let step = match kind {
            "delay" => Step::Delay { ms: item["ms"].as_u64().ok_or("délai invalide")?.min(600_000) },
            "hotkey" => {
                let keys = item["keys"].as_str().unwrap_or_default().trim().to_uppercase().replace(' ', "");
                if keys.is_empty() || keys.len() > 60 || !keys.chars().all(|c| c.is_ascii_alphanumeric() || "+-_".contains(c)) {
                    return Err(format!("raccourci invalide : {keys}"));
                }
                Step::Hotkey { keys }
            }
            "text" => Step::Text { text: item["text"].as_str().unwrap_or_default().chars().take(2000).collect() },
            "url" => {
                let url = item["url"].as_str().unwrap_or_default().trim().to_owned();
                if !(url.starts_with("https://") || url.starts_with("http://")) || url.contains(char::is_whitespace) {
                    return Err(format!("adresse non web refusée : {url}"));
                }
                Step::Url { url }
            }
            "launch" => {
                let program = item["program"].as_str().unwrap_or_default().trim().to_owned();
                if program.is_empty() {
                    return Err("programme vide".into());
                }
                let args = item["args"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).take(8).collect()).unwrap_or_default();
                Step::Launch { program, args, focus_if_running: false }
            }
            "media" => {
                let command = item["command"].as_str().unwrap_or_default();
                if !["play_pause", "next", "previous", "stop", "mute", "volume_up", "volume_down"].contains(&command) {
                    return Err(format!("commande média inconnue : {command}"));
                }
                Step::Media { command: command.into() }
            }
            "window_op" => {
                let op = item["op"].as_str().unwrap_or_default();
                if !["minimize", "maximize", "restore", "toggle_always_on_top", "close_others", "minimize_others"].contains(&op) {
                    return Err(format!("opération de fenêtre refusée : {op}"));
                }
                // Toujours la fenêtre active : jamais une cible devinée par le modèle.
                let mut step: Step = serde_json::from_value(json!({"type":"window_op","op":op})).map_err(|e| e.to_string())?;
                if let Step::WindowOp { target, .. } = &mut step {
                    *target = "foreground".into();
                }
                step
            }
            "close_window" => Step::CloseWindow,
            "notify" => serde_json::from_value(json!({"type":"notify","message":item["message"].as_str().unwrap_or_default().chars().take(500).collect::<String>()})).map_err(|e| e.to_string())?,
            other => return Err(format!("bloc non autorisé : {other}")),
        };
        steps.push(step);
    }
    Ok(steps)
}

/// Garde-fous indépendants du modèle : un petit modèle ajoute parfois des blocs non demandés.
/// - fermer une fenêtre exige un mot de fermeture dans la demande ;
/// - « le reste », « les autres »… n'autorisent jamais la fermeture de la fenêtre active ;
/// - un site n'est ouvert que si son nom figure dans la demande (pas d'adresse inventée) ;
/// - les blocs identiques consécutifs sont fusionnés.
pub fn guard(request: &str, steps: Vec<Step>) -> Vec<Step> {
    let low = request.to_lowercase();
    let compact: String = low.chars().filter(|c| !c.is_whitespace()).collect();
    let closing = ["ferm", "vire", "quitt", "close", "kill", "dégage", "degage", "enlève", "enleve"].iter().any(|w| low.contains(w));
    let others = ["autre", "reste", "sauf", "other", "except", "all but"].iter().any(|w| low.contains(w));
    let mut out: Vec<Step> = Vec::new();
    for step in steps {
        let keep = match &step {
            Step::CloseWindow => closing && !others,
            Step::WindowOp { op, .. } if op == "close_others" => closing,
            Step::Url { url } => {
                let host = url.split("://").nth(1).unwrap_or_default().trim_start_matches("www.");
                let name = host.split(['.', '/', ':']).next().unwrap_or_default().to_lowercase();
                name.len() >= 2 && compact.contains(&name)
            }
            _ => true,
        };
        if keep && out.last() != Some(&step) {
            out.push(step);
        }
    }
    out
}

/// Découpe `http://hôte:port[/chemin]` et refuse toute adresse non locale.
pub fn loopback(endpoint: &str) -> Result<(String, u16, String), String> {
    let rest = endpoint.trim().strip_prefix("http://").ok_or("adresse http:// locale attendue")?;
    let (authority, path) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
    let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let (h, p) = v6.split_once(']').ok_or("adresse IPv6 invalide")?;
        (h.to_owned(), p.strip_prefix(':').unwrap_or("80").to_owned())
    } else {
        let (h, p) = authority.rsplit_once(':').unwrap_or((authority, "80"));
        (h.to_owned(), p.to_owned())
    };
    let host = host.to_ascii_lowercase();
    if !["127.0.0.1", "localhost", "::1"].contains(&host.as_str()) {
        return Err("seules les adresses locales (127.0.0.1, localhost) sont acceptées".into());
    }
    let port = port.parse::<u16>().map_err(|_| "port invalide")?;
    Ok((if host == "localhost" { "127.0.0.1".into() } else { host }, port, format!("/{}", path.trim_end_matches('/')).trim_end_matches('/').to_owned()))
}

fn http(endpoint: &str, method: &str, path: &str, body: Option<&Value>, timeout: Duration) -> Result<Value, String> {
    let (host, port, prefix) = loopback(endpoint)?;
    let addr = (host.as_str(), port).to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("adresse introuvable")?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3))
        .map_err(|_| format!("aucun serveur IA local sur {endpoint} (Ollama est-il lancé ?)"))?;
    stream.set_read_timeout(Some(timeout)).ok();
    stream.set_write_timeout(Some(Duration::from_secs(10))).ok();
    let payload = body.map(|b| b.to_string()).unwrap_or_default();
    let request = format!(
        "{method} {prefix}{path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    stream.take(16 * 1024 * 1024).read_to_end(&mut raw).map_err(|e| {
        if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) { "l'IA locale n'a pas répondu à temps".to_owned() } else { e.to_string() }
    })?;
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or("réponse HTTP incomplète")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let mut body = raw[split + 4..].to_vec();
    let status: u16 = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    if head.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        body = dechunk(&body)?;
    }
    let text = String::from_utf8_lossy(&body);
    if !(200..300).contains(&status) {
        let detail = serde_json::from_str::<Value>(&text).ok()
            .and_then(|v| v["error"]["message"].as_str().or(v["error"].as_str()).map(str::to_owned))
            .unwrap_or_else(|| text.chars().take(200).collect());
        return Err(format!("erreur {status} du serveur IA local : {detail}"));
    }
    serde_json::from_str(&text).map_err(|e| format!("réponse illisible : {e}"))
}

fn dechunk(mut data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let line = data.windows(2).position(|w| w == b"\r\n").ok_or("bloc HTTP tronqué")?;
        let size = usize::from_str_radix(String::from_utf8_lossy(&data[..line]).split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| "taille de bloc HTTP invalide")?;
        data = &data[line + 2..];
        if size == 0 {
            return Ok(out);
        }
        if data.len() < size {
            return Err("bloc HTTP tronqué".into());
        }
        out.extend_from_slice(&data[..size]);
        data = data.get(size + 2..).unwrap_or_default();
    }
}

/// Liste les modèles exposés par le serveur local (`/v1/models`).
pub fn models(endpoint: &str) -> Result<Vec<String>, String> {
    let value = http(endpoint, "GET", "/v1/models", None, Duration::from_secs(10))?;
    Ok(value["data"].as_array().map(|list| list.iter().filter_map(|m| m["id"].as_str().map(str::to_owned)).collect()).unwrap_or_default())
}

fn complete(endpoint: &str, model: &str, request: &str) -> Result<Vec<Step>, String> {
    let body = json!({
        "model": model,
        "messages": messages(request),
        "temperature": 0,
        "max_tokens": 600,
        "stream": false,
        "response_format": {"type":"json_schema","json_schema":{"name":"deckforge_steps","strict":true,"schema":schema()}}
    });
    let value = http(endpoint, "POST", "/v1/chat/completions", Some(&body), Duration::from_secs(120))?;
    let content = value["choices"][0]["message"]["content"].as_str().ok_or("réponse sans contenu")?;
    parse_steps(content).map(|steps| guard(request, steps))
}

// ---------------------------------------------------------------- moteur intégré (option B)

/// Dossier du moteur intégré : `ai\` à côté de l'exécutable s'il y a été copié (version portable),
/// sinon `%LOCALAPPDATA%\DeckForge\ai` (toujours accessible en écriture, conservé par les mises à jour).
pub fn builtin_dir() -> PathBuf {
    let beside = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default().join("ai");
    if beside.join("llama-server.exe").is_file() {
        return beside;
    }
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).map_or(beside, |d| d.join("DeckForge").join("ai"))
}
pub fn builtin_server() -> PathBuf {
    builtin_dir().join("llama-server.exe")
}
pub fn builtin_model() -> PathBuf {
    builtin_dir().join("models").join(BUILTIN_MODEL_FILE)
}

// ---------------------------------------------------------------- installation à la demande (option B)

/// Fichiers épinglés (version et empreinte SHA-256 fixes) : rien n'est installé si l'empreinte diffère.
pub const ENGINE_URL: &str = "https://github.com/ggml-org/llama.cpp/releases/download/b11751/llama-b11751-bin-win-vulkan-x64.zip";
pub const ENGINE_SHA256: &str = "68aee00b9eca247ea5a2d1f04205c6afeec98974b6c7b2e0426b6ae6c9b3d729";
pub const ENGINE_SIZE: u64 = 36_751_747;
pub const MODEL_URL: &str = "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf";
pub const MODEL_SHA256: &str = "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e";
pub const MODEL_SIZE: u64 = 1_117_320_736;

/// Installation en cours ; l'interface lit l'étape, la taille déjà reçue et peut l'annuler.
pub struct Install {
    rx: Receiver<Result<(), String>>,
    pub stage: Arc<Mutex<(String, PathBuf, u64)>>,
    cancel: Arc<AtomicBool>,
}

impl Install {
    pub fn poll(&self) -> Option<Result<(), String>> {
        match self.rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("installation interrompue".into())),
        }
    }
    /// (étape, octets reçus, octets attendus).
    pub fn progress(&self) -> (String, u64, u64) {
        let (label, part, total) = self.stage.lock().map(|s| s.clone()).unwrap_or_default();
        (label, std::fs::metadata(&part).map_or(0, |m| m.len()), total)
    }
    /// Arrête le téléchargement ; le fichier partiel est gardé et la reprise repart de là.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

pub const CANCELLED: &str = "installation annulée ; elle reprendra là où elle s'est arrêtée";

/// Télécharge le moteur llama.cpp puis le modèle, uniquement sur clic explicite de l'utilisateur.
pub fn install_builtin() -> Install {
    let (tx, rx) = channel();
    let stage = Arc::new(Mutex::new((String::new(), PathBuf::new(), 0)));
    let cancel = Arc::new(AtomicBool::new(false));
    let (shared, stop) = (stage.clone(), cancel.clone());
    std::thread::spawn(move || {
        let set = |label: &str, part: &Path, total: u64| {
            if let Ok(mut s) = shared.lock() {
                *s = (label.into(), part.to_path_buf(), total);
            }
        };
        let result = install_into(&builtin_dir(), &set, &stop);
        let _ = tx.send(result);
    });
    Install { rx, stage, cancel }
}

/// Octets libres sur le volume de `dir` (None si inconnu).
pub fn free_space(dir: &Path) -> Option<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut free = 0u64;
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut())
        };
        (ok != 0).then_some(free)
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
        None
    }
}

/// Place encore nécessaire (fichiers manquants, déjà reçu déduit, marge de 200 Mo pour l'extraction).
pub fn space_needed(dir: &Path) -> u64 {
    let have = |p: PathBuf| std::fs::metadata(p).map_or(0, |m| m.len());
    let mut need = 200_000_000;
    if !dir.join("llama-server.exe").is_file() {
        need += ENGINE_SIZE.saturating_sub(have(dir.join("engine.zip.part"))) + 120_000_000;
    }
    let model = dir.join("models").join(BUILTIN_MODEL_FILE);
    if !model.is_file() {
        need += MODEL_SIZE.saturating_sub(have(model.with_extension("gguf.part")));
    }
    need
}

fn install_into(dir: &Path, set: &dyn Fn(&str, &Path, u64), cancel: &AtomicBool) -> Result<(), String> {
    std::fs::create_dir_all(dir.join("models")).map_err(|e| e.to_string())?;
    let need = space_needed(dir);
    if let Some(free) = free_space(dir) {
        if free < need {
            return Err(format!("espace disque insuffisant : {} Mo nécessaires, {} Mo libres", need / 1_000_000, free / 1_000_000));
        }
    }
    if !dir.join("llama-server.exe").is_file() {
        let zip = dir.join("engine.zip.part");
        set("Moteur llama.cpp", &zip, ENGINE_SIZE);
        fetch_verified(ENGINE_URL, &zip, ENGINE_SHA256, ENGINE_SIZE, 900, cancel)?;
        let extracted = extract_engine(&zip, dir);
        let _ = std::fs::remove_file(&zip);
        extracted?;
    }
    let model = dir.join("models").join(BUILTIN_MODEL_FILE);
    if !model.is_file() {
        let part = model.with_extension("gguf.part");
        set("Modèle Qwen2.5 1,5B", &part, MODEL_SIZE);
        fetch_verified(MODEL_URL, &part, MODEL_SHA256, MODEL_SIZE, 4 * 3600, cancel)?;
        std::fs::rename(&part, &model).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Télécharge avec reprise (`curl -C -`), annulable, `curl` rattaché à DeckForge (tué avec lui),
/// puis vérifie l'empreinte. Un fichier partiel est conservé pour la reprise ; un fichier complet
/// à l'empreinte différente est supprimé.
fn fetch_verified(url: &str, dest: &Path, sha256: &str, size: u64, seconds: u64, cancel: &AtomicBool) -> Result<(), String> {
    let have = std::fs::metadata(dest).map_or(0, |m| m.len());
    if have > size {
        let _ = std::fs::remove_file(dest);
    }
    if std::fs::metadata(dest).map_or(0, |m| m.len()) < size {
        let mut curl = std::process::Command::new("curl.exe");
        curl.args(["-L", "--fail", "--silent", "--show-error", "--retry", "3", "-C", "-", "-o"])
            .arg(dest)
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            curl.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut child = curl.spawn().map_err(|e| format!("téléchargement impossible (curl.exe : {e})"))?;
        #[cfg(windows)]
        let _job = kill_with_parent(&child);
        let started = Instant::now();
        loop {
            if cancel.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CANCELLED.into());
            }
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                if !status.success() {
                    return Err(format!("téléchargement interrompu ({status}) : vérifie la connexion Internet puis relance, il reprendra"));
                }
                break;
            }
            if started.elapsed() > Duration::from_secs(seconds) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("téléchargement trop long : relance-le, il reprendra".into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    let digest = crate::assets::sha256_file(dest).map_err(|e| e.to_string())?;
    if digest != sha256 {
        let _ = std::fs::remove_file(dest);
        return Err("fichier téléchargé corrompu (empreinte SHA-256 différente) : réessaie".into());
    }
    Ok(())
}

/// N'extrait que les fichiers à plat utiles au serveur (exécutable, DLL, licences).
fn extract_engine(zip: &Path, dir: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(zip).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut wanted = Vec::new();
    let mut server = None;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().map_err(|e| e.to_string())?.to_string();
        if entry.is_dir() || name.contains(['/', '\\', ':']) || name.contains("..") {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if lower == "llama-server.exe" {
            server = Some((i, name));
        } else if lower.ends_with(".dll") || lower.starts_with("license") {
            wanted.push((i, name));
        }
    }
    // Le serveur est écrit en dernier : sa présence garantit que les DLL sont complètes.
    wanted.push(server.ok_or("llama-server.exe absent de l'archive")?);
    for (i, name) in wanted {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let temporary = dir.join(format!("{name}.part"));
        std::io::copy(&mut entry, &mut std::fs::File::create(&temporary).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, dir.join(&name)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

struct Engine {
    child: std::process::Child,
    port: u16,
    id: u64,
    last_used: Instant,
    #[cfg(windows)]
    _job: Option<JobHandle>,
}

#[cfg(windows)]
struct JobHandle(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for JobHandle {}
#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}
#[cfg(windows)]
fn kill_with_parent(child: &std::process::Child) -> Option<JobHandle> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::*;
    unsafe {
        let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if handle.is_null() {
            return None;
        }
        let job = JobHandle(handle);
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = SetInformationJobObject(handle, JobObjectExtendedLimitInformation, &info as *const _ as *const _, std::mem::size_of_val(&info) as u32) != 0
            && AssignProcessToJobObject(handle, child.as_raw_handle() as _) != 0;
        ok.then_some(job)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Libération automatique de la mémoire après cette durée sans demande.
pub const IDLE_UNLOAD: Duration = Duration::from_secs(600);
/// Essais de démarrage : carte graphique d'abord, puis processeur seul si le moteur échoue
/// (mémoire vidéo insuffisante, pilote Vulkan absent ou défaillant).
const ATTEMPTS: &[(&str, &[&str])] = &[("carte graphique", &["-ngl", "99"]), ("processeur", &["-ngl", "0"])];

/// Moteur intégré partagé : démarré à la première demande, libéré après [`IDLE_UNLOAD`] sans
/// demande, arrêté avec DeckForge.
#[derive(Clone)]
pub struct Runtime {
    engine: Arc<Mutex<Option<Engine>>>,
    stop_requested: Arc<AtomicBool>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
    /// Demandes en cours : le moteur n'est jamais libéré pendant une réponse.
    busy: Arc<std::sync::atomic::AtomicUsize>,
    idle: Duration,
    /// Mode réellement utilisé au dernier démarrage (« carte graphique » ou « processeur »).
    pub mode: Arc<Mutex<String>>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::with_idle(IDLE_UNLOAD)
    }
}

impl Runtime {
    pub fn with_idle(idle: Duration) -> Self {
        Self {
            engine: Arc::default(),
            stop_requested: Arc::default(),
            next_id: Arc::default(),
            busy: Arc::default(),
            idle,
            mode: Arc::default(),
        }
    }

    /// Le moteur intégré est-il chargé en mémoire ?
    pub fn loaded(&self) -> bool {
        self.engine.try_lock().map(|g| g.is_some()).unwrap_or(true)
    }

    fn builtin_endpoint(&self) -> Result<String, String> {
        self.endpoint_with(ATTEMPTS)
    }

    fn endpoint_with(&self, attempts: &[(&str, &[&str])]) -> Result<String, String> {
        let mut guard = self.engine.lock().map_err(|_| "moteur indisponible")?;
        self.stop_requested.store(false, Ordering::SeqCst);
        if let Some(engine) = guard.as_mut() {
            if engine.child.try_wait().ok().flatten().is_none() {
                engine.last_used = Instant::now();
                return Ok(format!("http://127.0.0.1:{}", engine.port));
            }
            *guard = None;
        }
        let (server, model) = (builtin_server(), builtin_model());
        if !server.exists() {
            return Err("moteur IA intégré non installé : Assistant IA → IA locale → Installer".into());
        }
        if !model.exists() {
            return Err("modèle IA non installé : Assistant IA → IA locale → Installer".into());
        }
        let mut errors = Vec::new();
        for (index, (label, extra)) in attempts.iter().enumerate() {
            if self.stop_requested.load(Ordering::SeqCst) {
                return Err("moteur IA arrêté à la demande".into());
            }
            match self.launch(&server, &model, extra, index == 0) {
                Ok(engine) => {
                    if self.stop_requested.swap(false, Ordering::SeqCst) {
                        return Err("moteur IA arrêté à la demande".into());
                    }
                    let endpoint = format!("http://127.0.0.1:{}", engine.port);
                    if let Ok(mut mode) = self.mode.lock() {
                        *mode = (*label).into();
                    }
                    self.watch_idle(engine.id);
                    *guard = Some(engine);
                    return Ok(endpoint);
                }
                Err(e) => errors.push(format!("{label} : {e}")),
            }
        }
        Err(format!("le moteur IA n'a pas démarré ({}) ; détails dans {}", errors.join(" ; "), builtin_dir().join("llama-server.log").display()))
    }

    fn launch(&self, server: &Path, model: &Path, extra: &[&str], first: bool) -> Result<Engine, String> {
        let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map_err(|e| e.to_string())?.port();
        // Journal du moteur (écrasé au premier essai) pour comprendre un échec de démarrage.
        let log = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(!first)
            .truncate(first)
            .open(builtin_dir().join("llama-server.log"))
            .ok();
        let mut command = std::process::Command::new(server);
        command
            .args(["--host", "127.0.0.1", "--port", &port.to_string(), "-m"])
            .arg(model)
            .args(["-c", "4096", "--alias", "deckforge-local", "--no-webui"])
            .args(extra)
            .current_dir(builtin_dir())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(log.map_or_else(std::process::Stdio::null, std::process::Stdio::from));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let child = command.spawn().map_err(|e| format!("lancement impossible : {e}"))?;
        #[cfg(windows)]
        let job = kill_with_parent(&child);
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut engine = Engine { child, port, id, last_used: Instant::now(), #[cfg(windows)] _job: job };
        let endpoint = format!("http://127.0.0.1:{port}");
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            if let Ok(Some(status)) = engine.child.try_wait() {
                return Err(format!("arrêté dès le démarrage ({status})"));
            }
            if http(&endpoint, "GET", "/health", None, Duration::from_secs(2)).is_ok() {
                return Ok(engine);
            }
            if Instant::now() > deadline || self.stop_requested.load(Ordering::SeqCst) {
                return Err("pas prêt en 90 s".into());
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    /// Surveille l'inactivité du moteur `id` et le libère après `self.idle` sans demande.
    fn watch_idle(&self, id: u64) {
        let (engine, idle, busy) = (self.engine.clone(), self.idle, self.busy.clone());
        let tick = (idle / 4).clamp(Duration::from_millis(200), Duration::from_secs(30));
        std::thread::spawn(move || loop {
            std::thread::sleep(tick);
            let Ok(mut guard) = engine.lock() else { return };
            match guard.as_ref() {
                Some(e) if e.id == id => {
                    if e.last_used.elapsed() >= idle && busy.load(Ordering::SeqCst) == 0 {
                        *guard = None;
                        return;
                    }
                }
                _ => return,
            }
        });
    }

    /// Libère la mémoire (arrêt du moteur intégré) sans jamais bloquer l'interface : pendant un
    /// démarrage en cours, l'arrêt est différé à la fin du démarrage.
    pub fn stop(&self) {
        self.stop_requested.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.engine.try_lock() {
            *guard = None;
            self.stop_requested.store(false, Ordering::SeqCst);
        }
    }

    /// Traduit une demande ; l'inactivité est comptée à partir de la fin de la réponse.
    fn translate(&self, endpoint: &str, model: &str, request: &str) -> Result<Vec<Step>, String> {
        struct Busy<'a>(&'a Runtime);
        impl Drop for Busy<'_> {
            fn drop(&mut self) {
                if let Ok(mut guard) = self.0.engine.lock() {
                    if let Some(engine) = guard.as_mut() {
                        engine.last_used = Instant::now();
                    }
                }
                self.0.busy.fetch_sub(1, Ordering::SeqCst);
            }
        }
        self.busy.fetch_add(1, Ordering::SeqCst);
        let _busy = Busy(self);
        complete(endpoint, model, request)
    }

    fn resolve(&self, settings: &LocalAi) -> Result<(String, String), String> {
        if settings.backend == "integre" {
            Ok((self.builtin_endpoint()?, "deckforge-local".into()))
        } else {
            Ok((settings.endpoint.clone(), settings.model.clone()))
        }
    }

    /// Lance la traduction en arrière-plan ; l'interface interroge [`Job::poll`].
    pub fn spawn(&self, settings: &LocalAi, request: String) -> Job {
        let (tx, rx) = channel();
        let runtime = self.clone();
        let settings = settings.clone();
        std::thread::spawn(move || {
            let result = runtime.resolve(&settings).and_then(|(endpoint, model)| runtime.translate(&endpoint, &model, &request));
            let _ = tx.send(result);
        });
        Job { rx, started: Instant::now() }
    }

    /// Vérifie la connexion et la présence du modèle (bouton « Tester »).
    pub fn check(&self, settings: &LocalAi) -> Job {
        let (tx, rx) = channel();
        let runtime = self.clone();
        let settings = settings.clone();
        std::thread::spawn(move || {
            let result = runtime.resolve(&settings).and_then(|(endpoint, model)| {
                let list = models(&endpoint)?;
                if settings.backend != "integre" && !list.iter().any(|m| m == &model || m.trim_end_matches(":latest") == model) {
                    return Err(format!("modèle « {model} » absent. Dans un terminal : ollama pull {model}"));
                }
                runtime.translate(&endpoint, &model, "attends 1 seconde")
            });
            let _ = tx.send(result);
        });
        Job { rx, started: Instant::now() }
    }
}

pub struct Job {
    rx: Receiver<Result<Vec<Step>, String>>,
    pub started: Instant,
}

impl Job {
    pub fn poll(&self) -> Option<Result<Vec<Step>, String>> {
        match self.rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("traitement interrompu".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_endpoints_are_accepted() {
        assert_eq!(loopback("http://127.0.0.1:11434").unwrap(), ("127.0.0.1".into(), 11434, String::new()));
        assert_eq!(loopback("http://localhost:8080/").unwrap().0, "127.0.0.1");
        assert_eq!(loopback("http://[::1]:9000/api").unwrap(), ("::1".into(), 9000, "/api".into()));
        for bad in ["https://127.0.0.1:1", "http://192.168.0.2:11434", "http://example.com", "http://127.0.0.1.evil.com:80", "ftp://localhost"] {
            assert!(loopback(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn model_output_is_filtered_by_an_allow_list() {
        let steps = parse_steps(r#"Voici : {"steps":[{"type":"window_op","op":"close_others"},{"type":"delay","ms":2000},{"type":"hotkey","keys":"ctrl + shift + esc"}]}"#).unwrap();
        assert!(matches!(&steps[0], Step::WindowOp { op, target, .. } if op == "close_others" && target == "foreground"));
        assert_eq!(steps[1], Step::Delay { ms: 2000 });
        assert_eq!(steps[2], Step::Hotkey { keys: "CTRL+SHIFT+ESC".into() });
        for bad in [
            r#"{"steps":[{"type":"auto_hotkey","script":"x"}]}"#,
            r#"{"steps":[{"type":"http_request","url":"http://x"}]}"#,
            r#"{"steps":[{"type":"url","url":"file:///C:/Windows"}]}"#,
            r#"{"steps":[{"type":"window_op","op":"close"}]}"#,
            r#"{"steps":[{"type":"hotkey","keys":"CTRL+C & calc"}]}"#,
            "pas de json",
        ] {
            assert!(parse_steps(bad).is_err(), "{bad}");
        }
        assert!(parse_steps(r#"{"steps":[]}"#).unwrap().is_empty());
    }

    #[test]
    fn schema_examples_match_the_parser() {
        for (request, answer) in examples() {
            let text = answer.to_string();
            assert!(text.contains(r#"{"action":"#), "discriminant en premier : {text}");
            let steps = parse_steps(&text).unwrap();
            assert_eq!(guard(request, steps.clone()), steps, "les exemples passent les garde-fous : {request}");
        }
        assert_eq!(schema()["properties"]["steps"]["items"]["anyOf"].as_array().unwrap().len(), 9);
    }

    #[test]
    fn chunked_bodies_are_decoded() {
        assert_eq!(dechunk(b"4\r\nWiki\r\n5;x=1\r\npedia\r\n0\r\n\r\n").unwrap(), b"Wikipedia");
    }

    #[test]
    fn guard_drops_unrequested_closing_invented_sites_and_duplicates() {
        let win = |op: &str| parse_steps(&format!(r#"{{"steps":[{{"action":"window_op","op":"{op}"}}]}}"#)).unwrap().remove(0);
        let url = |u: &str| Step::Url { url: u.into() };
        assert_eq!(guard("garde juste cette fenetre et vire le reste", vec![win("maximize"), Step::CloseWindow]), vec![win("maximize")]);
        assert_eq!(guard("ferme tout sauf celle-ci", vec![win("close_others")]), vec![win("close_others")]);
        assert_eq!(guard("mets en plein ecran", vec![win("maximize"), win("close_others"), Step::CloseWindow]), vec![win("maximize")]);
        assert_eq!(guard("ferme cette fenetre", vec![Step::CloseWindow, Step::CloseWindow]), vec![Step::CloseWindow]);
        assert_eq!(guard("ouvre la calculatrice", vec![url("https://www.calculator.net")]), vec![]);
        assert_eq!(guard("ouvre you tube", vec![url("https://www.youtube.com/")]), vec![url("https://www.youtube.com/")]);
        assert_eq!(guard("ouvre twitch.tv", vec![url("https://twitch.tv")]), vec![url("https://twitch.tv")]);
    }

    #[test]
    fn engine_extraction_keeps_only_flat_server_files_and_writes_the_server_last() {
        use std::io::Write as _;
        let dir = std::env::temp_dir().join(format!("deckforge-ai-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("engine.zip");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, data) in [("llama-server.exe", "srv"), ("ggml-cpu.dll", "dll"), ("LICENSE-curl", "lic"), ("llama-cli.exe", "x"), ("sub/evil.dll", "x"), ("../up.dll", "x")] {
            writer.start_file(name, options).unwrap();
            writer.write_all(data.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
        extract_engine(&zip_path, &dir).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("llama-server.exe")).unwrap(), "srv");
        assert!(dir.join("ggml-cpu.dll").is_file() && dir.join("LICENSE-curl").is_file());
        assert!(!dir.join("llama-cli.exe").exists() && !dir.join("sub").exists() && !dir.parent().unwrap().join("up.dll").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Phrases libres (fautes, anglais) et suite exacte attendue (« bloc » ou « bloc:paramètre »,
    /// alternatives séparées par « | » ; « ask » : une question de précision des règles est correcte).
    /// Les dix dernières ne ressemblent à aucun exemple du prompt.
    const LAB_REQUESTS: &[(&str, &[&str])] = &[
        ("garde juste la fenêtre où je suis et vire le reste", &["window_op:close_others"]),
        ("mets la musique en pause puis ouvre google", &["media:play_pause", "url"]),
        ("lock my computer", &["hotkey:WIN+L"]),
        ("ouvre le bloc note", &["launch:notepad.exe"]),
        ("monte le son", &["media:volume_up"]),
        ("chanson suivante stp", &["media:next"]),
        ("fais une capture d'ecran", &["hotkey:WIN+SHIFT+S"]),
        ("ecris mon adresse mail hatim@example.com", &["text"]),
        ("ouvre youtube", &["url"]),
        ("reduis toutes les autres fenetres", &["window_op:minimize_others"]),
        ("met la fenetre en plein ecran", &["window_op:maximize"]),
        ("ferme cette fenetre", &["close_window"]),
        ("attends 3 secondes puis copie", &["delay", "hotkey:CTRL+C"]),
        ("show the desktop", &["hotkey:WIN+D"]),
        ("open the task manager", &["hotkey:CTRL+SHIFT+ESC"]),
        ("coupe le son", &["media:mute|ask"]),
        ("rappelle moi de boire de l'eau", &["notify"]),
        ("garde cette fenetre toujours au dessus", &["window_op:toggle_always_on_top"]),
        ("ouvre la calculatrice", &["launch:calc.exe"]),
        ("colle sans mise en forme", &["hotkey:CTRL+SHIFT+V"]),
        ("verrouille le pc", &["hotkey:WIN+L"]),
        ("ouvre l'explorateur de fichiers", &["launch:explorer.exe|hotkey:WIN+E"]),
        ("baisse le volume", &["media:volume_down|ask"]),
        ("musique précédente", &["media:previous"]),
        ("tape merci beaucoup", &["text"]),
        ("ouvre twitch.tv", &["url"]),
        ("annule la derniere action", &["hotkey:CTRL+Z"]),
        ("agrandis la fenetre", &["window_op:maximize"]),
        ("previens moi que la pizza est prete", &["notify"]),
        ("wait half a second then press enter", &["delay", "hotkey:ENTER"]),
    ];

    fn signature(step: &Step) -> String {
        let v = serde_json::to_value(step).unwrap_or_default();
        let kind = v["type"].as_str().unwrap_or_default().to_owned();
        let detail = match kind.as_str() {
            "window_op" => v["op"].as_str().map(str::to_owned),
            "media" => v["command"].as_str().map(str::to_owned),
            "hotkey" => v["keys"].as_str().map(str::to_owned),
            "launch" => v["program"].as_str().map(str::to_lowercase),
            _ => None,
        };
        detail.map_or(kind.clone(), |d| format!("{kind}:{d}"))
    }

    fn matches(steps: &[Step], expected: &[&str]) -> bool {
        // « Coller sans mise en forme » : le préréglage du catalogue (3 blocs) est aussi juste.
        let preset = crate::catalog::presets().into_iter().find(|p| p.key.label == "Coller sans mise en forme").map(|p| p.key.steps);
        if expected == ["hotkey:CTRL+SHIFT+V"] && preset.as_deref() == Some(steps) {
            return true;
        }
        steps.len() == expected.len()
            && steps.iter().zip(expected).all(|(s, e)| {
                let sig = signature(s);
                e.split('|').any(|e| sig == e || (!e.contains(':') && sig.split(':').next() == Some(e)))
            })
    }

    /// Score strict du modèle seul, puis de la chaîne réelle de l'atelier : règles d'abord,
    /// IA locale seulement si les règles ne reconnaissent pas la demande.
    fn run_lab(endpoint: &str, model: &str) {
        let config = crate::model::Config::default();
        let (mut alone, mut chain, mut total_ms, mut by_ai) = (0, 0, 0u128, 0);
        for (request, expected) in LAB_REQUESTS {
            let started = Instant::now();
            let result = complete(endpoint, model, request);
            let ms = started.elapsed().as_millis();
            total_ms += ms;
            let model_steps = result.clone().unwrap_or_default();
            alone += usize::from(matches(&model_steps, expected));
            let rules = crate::local_assistant::propose(request, &config);
            let (source, steps) = if matches!(rules.status, crate::local_assistant::Status::Inconnue) {
                by_ai += 1;
                ("IA    ", crate::local_assistant::from_model(model_steps, &config).steps)
            } else {
                ("règles", rules.steps)
            };
            let asks = matches!(rules.status, crate::local_assistant::Status::Ambigue { .. }) && expected.iter().any(|e| e.split('|').any(|e| e == "ask"));
            let ok = asks || matches(&steps, expected);
            chain += usize::from(ok);
            let shown: Vec<String> = steps.iter().map(signature).collect();
            eprintln!("LAB {} {source} | {request} -> {shown:?} ({ms} ms){}", if ok { "OK  " } else { "DIFF" }, result.err().map(|e| format!(" erreur : {e}")).unwrap_or_default());
        }
        let n = LAB_REQUESTS.len();
        eprintln!("LAB score modèle seul {alone}/{n} · chaîne atelier {chain}/{n} ({by_ai} demandes passées à l'IA) · moyenne {} ms", total_ms / n as u128);
        assert!(alone * 10 >= n * 7, "modèle seul : moins de 70 % de suites exactes");
        assert!(chain * 10 >= n * 8, "chaîne atelier : moins de 80 % de suites exactes");
    }

    /// Banc réel option A : `DECKFORGE_LOCAL_AI=http://127.0.0.1:11434 DECKFORGE_LOCAL_AI_MODEL=qwen2.5:1.5b`.
    #[test]
    #[ignore]
    fn lab_local_model_translates_natural_requests() {
        let endpoint = std::env::var("DECKFORGE_LOCAL_AI").expect("DECKFORGE_LOCAL_AI");
        let model = std::env::var("DECKFORGE_LOCAL_AI_MODEL").unwrap_or_else(|_| OLLAMA_MODEL.into());
        run_lab(&endpoint, &model);
    }

    /// Serveur HTTP local d'un seul fichier, avec prise en charge des plages (`Range: bytes=N-`).
    fn range_server(body: &'static [u8]) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut request = [0u8; 2048];
                let n = stream.read(&mut request).unwrap_or(0);
                let head = String::from_utf8_lossy(&request[..n]).to_lowercase();
                let start = head.lines().find_map(|l| l.strip_prefix("range: bytes=")).and_then(|r| r.trim_end_matches('-').trim().trim_end_matches('-').parse::<usize>().ok()).unwrap_or(0);
                let part = &body[start.min(body.len())..];
                let status = if start > 0 { format!("206 Partial Content\r\nContent-Range: bytes {start}-{}/{}", body.len() - 1, body.len()) } else { "200 OK".into() };
                let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n", part.len());
                let _ = stream.write_all(part);
            }
        });
        format!("http://127.0.0.1:{port}/fichier.bin")
    }

    #[cfg(windows)]
    #[test]
    fn download_resumes_verifies_checksum_and_can_be_cancelled() {
        use sha2::{Digest, Sha256};
        const BODY: &[u8] = b"DeckForge : contenu de test pour la reprise du telechargement.";
        let sha: String = Sha256::digest(BODY).iter().map(|b| format!("{b:02x}")).collect();
        let url = range_server(BODY);
        let dir = std::env::temp_dir().join(format!("deckforge-dl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("fichier.part");
        let never = AtomicBool::new(false);
        // Reprise : la première moitié est déjà là.
        std::fs::write(&dest, &BODY[..20]).unwrap();
        fetch_verified(&url, &dest, &sha, BODY.len() as u64, 30, &never).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), BODY);
        // Fichier complet : aucune requête, simple vérification.
        fetch_verified("http://127.0.0.1:1/injoignable", &dest, &sha, BODY.len() as u64, 30, &never).unwrap();
        // Empreinte différente : le fichier est supprimé.
        assert!(fetch_verified(&url, &dest, &"0".repeat(64), BODY.len() as u64, 30, &never).is_err());
        assert!(!dest.exists());
        // Annulation : arrêt immédiat, sans fichier corrompu vérifié.
        let cancelled = AtomicBool::new(true);
        assert_eq!(fetch_verified(&url, &dest, &sha, BODY.len() as u64, 30, &cancelled).unwrap_err(), CANCELLED);
        // Espace nécessaire : tout manque au départ, la partie déjà reçue est déduite.
        let empty = dir.join("vide");
        assert_eq!(space_needed(&empty), 200_000_000 + ENGINE_SIZE + 120_000_000 + MODEL_SIZE);
        std::fs::create_dir_all(empty.join("models")).unwrap();
        std::fs::write(empty.join("models").join(BUILTIN_MODEL_FILE).with_extension("gguf.part"), vec![0u8; 1_000_000]).unwrap();
        assert_eq!(space_needed(&empty), 200_000_000 + ENGINE_SIZE + 120_000_000 + MODEL_SIZE - 1_000_000);
        assert!(free_space(&dir).is_some_and(|free| free > 0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Banc réel : repli sur le processeur quand le premier démarrage échoue, puis libération
    /// automatique de la mémoire après inactivité.
    #[test]
    #[ignore]
    fn lab_builtin_engine_falls_back_to_cpu_and_unloads_when_idle() {
        let runtime = Runtime::with_idle(Duration::from_secs(3));
        let endpoint = runtime.endpoint_with(&[("carte graphique", &["--option-inexistante"]), ("processeur", &["-ngl", "0"])]).unwrap();
        assert_eq!(runtime.mode.lock().unwrap().as_str(), "processeur");
        let log = std::fs::read_to_string(builtin_dir().join("llama-server.log")).unwrap_or_default();
        eprintln!("LAB journal du moteur : {} octets, mentionne l'option refusée : {}", log.len(), log.contains("option-inexistante"));
        // Demande plus longue que le délai d'inactivité : le moteur ne doit pas être coupé en route.
        let steps = runtime.translate(&endpoint, "deckforge-local", "ferme cette fenetre").unwrap();
        eprintln!("LAB repli processeur -> {steps:?}");
        assert!(runtime.loaded());
        std::thread::sleep(Duration::from_secs(6));
        assert!(!runtime.loaded(), "moteur libéré après inactivité");
        assert!(http(&endpoint, "GET", "/health", None, Duration::from_secs(2)).is_err(), "processus arrêté");
        // Une nouvelle demande le redémarre.
        let again = runtime.endpoint_with(ATTEMPTS).unwrap();
        assert!(http(&again, "GET", "/health", None, Duration::from_secs(2)).is_ok());
        runtime.stop();
    }

    /// Banc réel : installation par le vrai code (téléchargement épinglé, SHA-256, extraction).
    #[test]
    #[ignore]
    fn lab_install_builtin_downloads_and_verifies() {
        let started = Instant::now();
        let install = install_builtin();
        let result = loop {
            if let Some(result) = install.poll() { break result; }
            std::thread::sleep(Duration::from_millis(500));
        };
        eprintln!("LAB installation {result:?} en {} s dans {}", started.elapsed().as_secs(), builtin_dir().display());
        result.unwrap();
        assert!(builtin_server().is_file() && builtin_model().is_file());
    }

    /// Banc réel option B : moteur et modèle installés dans `builtin_dir()`, démarrés par DeckForge.
    #[test]
    #[ignore]
    fn lab_builtin_engine_starts_translates_and_stops() {
        let runtime = Runtime::default();
        let started = Instant::now();
        let endpoint = runtime.builtin_endpoint().unwrap();
        eprintln!("LAB moteur intégré prêt sur {endpoint} en {} ms", started.elapsed().as_millis());
        run_lab(&endpoint, "deckforge-local");
        runtime.stop();
        assert!(http(&endpoint, "GET", "/health", None, Duration::from_secs(2)).is_err(), "le moteur doit être arrêté");
    }
}
