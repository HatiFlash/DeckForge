//! OBS telemetry runs independently of the window and its App mutex.
use crate::model::ObsSettings;
use serde_json::Value;
use std::{sync::{mpsc::{self, Sender, Receiver}, Mutex, OnceLock}, time::{Duration, Instant}};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub status: String,
    pub record: bool,
    pub stream: bool,
    pub cam: bool,
    pub replay: bool,
    pub scene: String,
    pub muted: std::collections::HashMap<String, bool>,
    pub scenes: Vec<String>,
    pub inputs: Vec<String>,
    pub scene_items: std::collections::HashMap<String, Vec<String>>,
}
struct Monitor { tx: Sender<(ObsSettings, bool)>, state: std::sync::Arc<Mutex<Snapshot>> }
static MONITOR: OnceLock<Monitor> = OnceLock::new();
fn monitor() -> &'static Monitor {
    MONITOR.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let state = std::sync::Arc::new(Mutex::new(Snapshot { status: "OBS : déconnecté".into(), ..Default::default() }));
        let worker_state = state.clone();
        std::thread::spawn(move || run(rx, worker_state));
        Monitor { tx, state }
    })
}
pub fn update(settings: &ObsSettings, force: bool) {
    let _ = monitor().tx.send((settings.clone(), force));
}
pub fn snapshot() -> Snapshot {
    monitor().state.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
fn delay(attempt: usize) -> Duration {
    Duration::from_secs(match attempt { 0 => 2, 1 => 5, 2 => 10, _ => 30 })
}
fn apply(s: &mut Snapshot, kind: &str, data: &Value) {
    match kind {
        "GetRecordStatus" | "RecordStateChanged" => if let Some(v) = data["outputActive"].as_bool() { s.record = v; },
        "GetStreamStatus" | "StreamStateChanged" => if let Some(v) = data["outputActive"].as_bool() { s.stream = v; },
        "GetVirtualCamStatus" | "VirtualcamStateChanged" => if let Some(v) = data["outputActive"].as_bool() { s.cam = v; },
        "GetReplayBufferStatus" | "ReplayBufferStateChanged" => if let Some(v) = data["outputActive"].as_bool() { s.replay = v; },
        "GetCurrentProgramScene" | "CurrentProgramSceneChanged" => if let Some(v) = data["currentProgramSceneName"].as_str().or_else(|| data["sceneName"].as_str()) { s.scene = v.into(); },
        "GetInputMute" | "InputMuteStateChanged" => if let (Some(name), Some(mute)) = (data["inputName"].as_str(), data["inputMuted"].as_bool()) { s.muted.insert(name.into(), mute); },
        "GetSceneList" => if let Some(xs) = data["scenes"].as_array() { s.scenes = xs.iter().filter_map(|x| x["sceneName"].as_str().map(str::to_owned)).collect(); },
        "GetSceneItemList" => if let (Some(name), Some(xs)) = (data["sceneName"].as_str(), data["sceneItems"].as_array()) { s.scene_items.insert(name.into(), xs.iter().filter_map(|x| x["sourceName"].as_str().map(str::to_owned)).collect()); },
        "GetInputList" => if let Some(xs) = data["inputs"].as_array() { s.inputs = xs.iter().filter_map(|x| x["inputName"].as_str().map(str::to_owned)).collect(); },
        _ => {}
    }
}
fn connected(settings: &ObsSettings, state: &Mutex<Snapshot>, rx: &Receiver<(ObsSettings, bool)>) -> anyhow::Result<Option<ObsSettings>> {
    let mut ws = crate::obs::socket(settings)?;
    let version = crate::obs::request(&mut ws, "monitor-version", "GetVersion", serde_json::json!({}))?;
    let mut next = Snapshot { status: format!("OBS : connecté {} / obs-websocket {}", version["obsVersion"].as_str().unwrap_or("?"), version["obsWebSocketVersion"].as_str().unwrap_or("?")), ..Default::default() };
    for kind in ["GetRecordStatus", "GetStreamStatus", "GetVirtualCamStatus", "GetReplayBufferStatus", "GetCurrentProgramScene", "GetSceneList", "GetInputList"] {
        if let Ok(data) = crate::obs::request(&mut ws, kind, kind, serde_json::json!({})) { apply(&mut next, kind, &data); }
    }
    for scene in next.scenes.clone() {
        if let Ok(mut data) = crate::obs::request(&mut ws, &format!("items-{scene}"), "GetSceneItemList", serde_json::json!({"sceneName":scene})) {
            data["sceneName"] = scene.into(); apply(&mut next, "GetSceneItemList", &data);
        }
    }
    for input in next.inputs.clone() {
        if let Ok(mut data) = crate::obs::request(&mut ws, &format!("mute-{input}"), "GetInputMute", serde_json::json!({"inputName":input})) {
            data["inputName"] = input.into(); apply(&mut next, "GetInputMute", &data);
        }
    }
    *state.lock().unwrap_or_else(|e| e.into_inner()) = next;
    ws.get_mut().set_read_timeout(Some(Duration::from_millis(400)))?;
    loop {
        if let Ok((other, force)) = rx.try_recv() { if force || other.host != settings.host || other.port != settings.port || other.password != settings.password { return Ok(Some(other)); } }
        match ws.read() {
            Ok(tungstenite::Message::Text(text)) => {
                let packet: Value = serde_json::from_str(&text)?;
                if packet["op"] == 5 {
                    let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                    apply(&mut s, packet["d"]["eventType"].as_str().unwrap_or(""), &packet["d"]["eventData"]);
                }
            },
            Err(tungstenite::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::TimedOut || e.kind() == std::io::ErrorKind::WouldBlock => {},
            Err(e) => return Err(e.into()),
            _ => {}
        }
    }
}
fn run(rx: Receiver<(ObsSettings, bool)>, state: std::sync::Arc<Mutex<Snapshot>>) {
    let mut current: Option<ObsSettings> = None;
    let mut attempt = 0;
    let mut due = Instant::now();
    loop {
        let wait = due.saturating_duration_since(Instant::now()).min(Duration::from_millis(400));
        match rx.recv_timeout(wait) {
            Ok((settings, force)) => {
                if force || current.as_ref().is_none_or(|s| s.host != settings.host || s.port != settings.port || s.password != settings.password) {
                    current = Some(settings); attempt = 0; due = Instant::now();
                }
                while let Ok((settings, force)) = rx.try_recv() {
                    if force || current.as_ref().is_none_or(|s| s.host != settings.host || s.port != settings.port || s.password != settings.password) {
                        current = Some(settings); attempt = 0; due = Instant::now();
                    }
                }
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if Instant::now() < due { continue; }
        let Some(settings) = current.clone() else { due = Instant::now() + Duration::from_secs(1); continue; };
        let outcome = connected(&settings, &state, &rx);
        if let Ok(Some(other)) = outcome { current = Some(other); attempt = 0; due = Instant::now(); continue; }
        let status = if outcome.as_ref().err().is_some_and(|e| e.to_string().contains("Authentification OBS refusée")) { "OBS : mot de passe refusé" } else { "OBS : déconnecté" };
        *state.lock().unwrap_or_else(|e| e.into_inner()) = Snapshot { status: status.into(), ..Default::default() };
        due = Instant::now() + delay(attempt);
        attempt = attempt.saturating_add(1);
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn fake_server_initial_state_and_record_event() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let send = |ws: &mut tungstenite::WebSocket<std::net::TcpStream>, packet: Value| {
                ws.send(tungstenite::Message::Text(packet.to_string().into())).unwrap();
            };
            send(&mut ws, serde_json::json!({"op":0,"d":{"rpcVersion":1}}));
            let identify: Value = serde_json::from_str(&ws.read().unwrap().into_text().unwrap()).unwrap();
            assert_eq!(identify["d"]["eventSubscriptions"], 205);
            send(&mut ws, serde_json::json!({"op":2,"d":{"negotiatedRpcVersion":1}}));
            for _ in 0..9 {
                let request: Value = serde_json::from_str(&ws.read().unwrap().into_text().unwrap()).unwrap();
                let kind = request["d"]["requestType"].as_str().unwrap();
                let data = match kind {
                    "GetVersion" => serde_json::json!({"obsVersion":"31.0","obsWebSocketVersion":"5.5"}),
                    "GetRecordStatus" => serde_json::json!({"outputActive":false}),
                    "GetCurrentProgramScene" => serde_json::json!({"currentProgramSceneName":"Bureau"}),
                    "GetSceneList" => serde_json::json!({"scenes":[{"sceneName":"Bureau"}]}),
                    "GetInputList" => serde_json::json!({"inputs":[]}),
                    "GetSceneItemList" => serde_json::json!({"sceneItems":[]}),
                    _ => serde_json::json!({"outputActive":false}),
                };
                send(&mut ws, serde_json::json!({"op":7,"d":{"requestId":request["d"]["requestId"],"requestStatus":{"result":true},"responseData":data}}));
            }
            send(&mut ws, serde_json::json!({"op":5,"d":{"eventType":"RecordStateChanged","eventData":{"outputActive":true}}}));
            std::thread::sleep(Duration::from_millis(250));
        });
        let state = Mutex::new(Snapshot::default());
        let (tx, rx) = mpsc::channel();
        let settings = ObsSettings { host:"127.0.0.1".into(), port, password:String::new() };
        let task = std::thread::spawn(move || { let _ = connected(&settings, &state, &rx); state.into_inner().unwrap() });
        std::thread::sleep(Duration::from_millis(80));
        tx.send((ObsSettings { host:"127.0.0.1".into(), port, password:String::new() }, true)).unwrap();
        let final_state = task.join().unwrap();
        server.join().unwrap();
        assert!(final_state.status.contains("31.0 / obs-websocket 5.5"));
        assert_eq!(final_state.scene, "Bureau");
        assert!(final_state.record);
    }
    #[test] fn retry_intervals() { assert_eq!((0..5).map(|i|delay(i).as_secs()).collect::<Vec<_>>(), vec![2,5,10,30,30]); }
    #[test] fn event_updates_only_changed_state() {
        let mut s = Snapshot::default(); apply(&mut s,"RecordStateChanged",&serde_json::json!({"outputActive":true})); assert!(s.record);
        apply(&mut s,"InputMuteStateChanged",&serde_json::json!({"inputName":"Mic","inputMuted":true})); assert_eq!(s.muted.get("Mic"),Some(&true));
    }
}



/// Only the transient presentation changes; configured actions and serialised profiles stay intact.
pub fn present_key(key: &crate::model::Key, state: &Snapshot) -> crate::model::Key {
    let mut shown = key.clone();
    if key.steps.len() != 1 { return shown; }
    let active = match key.steps[0].active() {
        crate::model::Step::ObsRecord { .. } if state.record => Some("REC"),
        crate::model::Step::ObsStream { .. } if state.stream => Some("LIVE"),
        crate::model::Step::ObsVirtualCam { .. } if state.cam => Some("CAM"),
        crate::model::Step::ObsReplay { .. } if state.replay => Some("REPLAY"),
        crate::model::Step::ObsScene { name } if !state.scene.is_empty() && *name == state.scene => Some("SCÈNE"),
        crate::model::Step::ObsMute { input, .. } if state.muted.get(input) == Some(&true) => Some("×"),
        _ => None,
    };
    if let Some(badge) = active { shown.runtime_badge = Some(badge.into()); }
    shown
}
#[cfg(test)] mod presentation_tests {
    use super::*;
    #[test] fn record_badge_follows_event_and_never_serializes() {
        let key = crate::model::Key { label:"Studio".into(), steps: vec![crate::model::Step::ObsRecord { mode:"toggle".into() }], ..Default::default() };
        let mut state = Snapshot::default();
        assert!(present_key(&key, &state).runtime_badge.is_none());
        apply(&mut state,"RecordStateChanged", &serde_json::json!({"outputActive":true}));
        let shown = present_key(&key, &state);
        assert_eq!(shown.runtime_badge.as_deref(), Some("REC"));
        assert_eq!(shown.label, "Studio");
        assert!(!serde_json::to_string(&shown).unwrap().contains("runtime_badge"));
    }
}
