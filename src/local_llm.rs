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
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Modèle conseillé : Qwen2.5 1,5B Instruct (Apache 2.0), environ 1 Go en Q4.
pub const OLLAMA_MODEL: &str = "qwen2.5:1.5b";
pub const BUILTIN_MODEL_FILE: &str = "qwen2.5-1.5b-instruct-q4_k_m.gguf";
const MAX_STEPS: usize = 8;

const SYSTEM: &str = "Tu convertis une demande (français ou anglais, souvent avec des fautes) en blocs \
d'automatisation DeckForge pour Windows. Réponds uniquement avec un objet JSON {\"steps\":[...]}.\n\
Blocs autorisés :\n\
- {\"type\":\"delay\",\"ms\":1000} attendre.\n\
- {\"type\":\"hotkey\",\"keys\":\"CTRL+C\"} raccourci clavier ; touches en majuscules séparées par +, ex. WIN+D, ALT+F4, CTRL+SHIFT+ESC, ALT+TAB, WIN+SHIFT+S, F5.\n\
- {\"type\":\"text\",\"text\":\"bonjour\"} taper un texte.\n\
- {\"type\":\"url\",\"url\":\"https://...\"} ouvrir une adresse web.\n\
- {\"type\":\"launch\",\"program\":\"notepad.exe\",\"args\":[]} lancer un programme connu ; n'invente jamais de chemin complet.\n\
- {\"type\":\"media\",\"command\":\"play_pause\"} commandes : play_pause, next, previous, stop, mute, volume_up, volume_down.\n\
- {\"type\":\"window_op\",\"op\":\"minimize\"} fenêtre active ; op : minimize, maximize, restore, toggle_always_on_top, close_others (fermer toutes les autres fenêtres), minimize_others (réduire toutes les autres).\n\
- {\"type\":\"close_window\"} fermer la fenêtre active.\n\
- {\"type\":\"notify\",\"message\":\"...\"} afficher une notification.\n\
Garde l'ordre de la demande. Si rien ne correspond, réponds {\"steps\":[]}.";

const EXAMPLES: &[(&str, &str)] = &[
    (
        "ferme tout sauf la fenetre ou je suis puis attend 2 secondes",
        r#"{"steps":[{"type":"window_op","op":"close_others"},{"type":"delay","ms":2000}]}"#,
    ),
    (
        "open youtube and pause the music",
        r#"{"steps":[{"type":"url","url":"https://www.youtube.com"},{"type":"media","command":"play_pause"}]}"#,
    ),
    (
        "copie, attends une demi seconde et colle",
        r#"{"steps":[{"type":"hotkey","keys":"CTRL+C"},{"type":"delay","ms":500},{"type":"hotkey","keys":"CTRL+V"}]}"#,
    ),
];

/// Schéma JSON contraignant la génération (grammaire côté llama.cpp / Ollama).
pub fn schema() -> Value {
    let obj = |kind: &str, props: Value, required: &[&str]| {
        let mut properties = json!({"type": {"const": kind}});
        for (k, v) in props.as_object().unwrap() {
            properties[k] = v.clone();
        }
        let mut req = vec!["type"];
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
    for (user, answer) in EXAMPLES {
        list.push(json!({"role":"user","content":user}));
        list.push(json!({"role":"assistant","content":answer}));
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
        let kind = item["type"].as_str().unwrap_or_default();
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
    parse_steps(content)
}

// ---------------------------------------------------------------- moteur intégré (option B)

/// Dossier du moteur intégré : `ai\` à côté de l'exécutable (installateur ou version portable).
pub fn builtin_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default().join("ai")
}
pub fn builtin_server() -> PathBuf {
    builtin_dir().join("llama-server.exe")
}
pub fn builtin_model() -> PathBuf {
    builtin_dir().join("models").join(BUILTIN_MODEL_FILE)
}

struct Engine {
    child: std::process::Child,
    port: u16,
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

/// Moteur intégré partagé, démarré à la première demande et arrêté avec DeckForge.
#[derive(Clone, Default)]
pub struct Runtime {
    engine: Arc<Mutex<Option<Engine>>>,
}

impl Runtime {
    fn builtin_endpoint(&self) -> Result<String, String> {
        let mut guard = self.engine.lock().map_err(|_| "moteur indisponible")?;
        if let Some(engine) = guard.as_mut() {
            if engine.child.try_wait().ok().flatten().is_none() {
                return Ok(format!("http://127.0.0.1:{}", engine.port));
            }
            *guard = None;
        }
        let (server, model) = (builtin_server(), builtin_model());
        if !server.exists() {
            return Err("moteur IA intégré absent (dossier ai\\ de DeckForge)".into());
        }
        if !model.exists() {
            return Err("modèle IA non téléchargé : Réglages → IA locale → Télécharger le modèle".into());
        }
        let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map_err(|e| e.to_string())?.port();
        let mut command = std::process::Command::new(&server);
        command
            .args(["--host", "127.0.0.1", "--port", &port.to_string(), "-m"])
            .arg(&model)
            .args(["-c", "4096", "-ngl", "99", "--alias", "deckforge-local", "--no-webui"])
            .current_dir(builtin_dir())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let child = command.spawn().map_err(|e| format!("démarrage du moteur IA impossible : {e}"))?;
        #[cfg(windows)]
        let job = kill_with_parent(&child);
        let mut engine = Engine { child, port, #[cfg(windows)] _job: job };
        let endpoint = format!("http://127.0.0.1:{port}");
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            if let Ok(Some(status)) = engine.child.try_wait() {
                return Err(format!("le moteur IA s'est arrêté ({status})"));
            }
            if http(&endpoint, "GET", "/health", None, Duration::from_secs(2)).is_ok() {
                break;
            }
            if Instant::now() > deadline {
                return Err("le moteur IA n'a pas démarré en 90 s".into());
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        *guard = Some(engine);
        Ok(endpoint)
    }

    /// Libère la mémoire vidéo (arrêt du moteur intégré).
    pub fn stop(&self) {
        if let Ok(mut guard) = self.engine.lock() {
            *guard = None;
        }
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
            let result = runtime.resolve(&settings).and_then(|(endpoint, model)| complete(&endpoint, &model, &request));
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
                complete(&endpoint, &model, "attends 1 seconde")
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
        for (_, answer) in EXAMPLES {
            assert!(!parse_steps(answer).unwrap().is_empty());
        }
        assert_eq!(schema()["properties"]["steps"]["items"]["anyOf"].as_array().unwrap().len(), 9);
    }

    #[test]
    fn chunked_bodies_are_decoded() {
        assert_eq!(dechunk(b"4\r\nWiki\r\n5;x=1\r\npedia\r\n0\r\n\r\n").unwrap(), b"Wikipedia");
    }

    /// Banc réel : `DECKFORGE_LOCAL_AI=http://127.0.0.1:11434 DECKFORGE_LOCAL_AI_MODEL=qwen2.5:1.5b`.
    #[test]
    #[ignore]
    fn lab_local_model_translates_natural_requests() {
        let endpoint = std::env::var("DECKFORGE_LOCAL_AI").unwrap();
        let model = std::env::var("DECKFORGE_LOCAL_AI_MODEL").unwrap_or_else(|_| OLLAMA_MODEL.into());
        for request in [
            "garde juste la fenêtre où je suis et vire le reste",
            "mets la musique en pause puis ouvre google",
            "lock my computer",
        ] {
            let started = Instant::now();
            let steps = complete(&endpoint, &model, request).unwrap();
            eprintln!("{request} -> {steps:?} ({} ms)", started.elapsed().as_millis());
            assert!(!steps.is_empty(), "{request}");
        }
    }
}
