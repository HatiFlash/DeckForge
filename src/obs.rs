//! obs-websocket v5: local connection, authenticated handshake and bounded requests.
use crate::model::{ObsSettings, Step};
use anyhow::{bail, Context, Result};
use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{net::TcpStream, time::Duration, sync::{Mutex, OnceLock}};
use tungstenite::{client, Message, WebSocket};

fn authentication(password: &str, salt: &str, challenge: &str) -> String {
    let first = Sha256::digest(format!("{password}{salt}").as_bytes());
    let secret = base64::engine::general_purpose::STANDARD.encode(first);
    base64::engine::general_purpose::STANDARD.encode(Sha256::digest(format!("{secret}{challenge}").as_bytes()))
}
pub(crate) fn socket(settings: &ObsSettings) -> Result<WebSocket<TcpStream>> {
    anyhow::ensure!(settings.host == "localhost" || settings.host == "127.0.0.1", "OBS local uniquement");
    let stream = TcpStream::connect_timeout(&format!("127.0.0.1:{}", settings.port).parse()?, Duration::from_secs(2)).context("OBS inaccessible")?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let (mut ws, _) = client(format!("ws://127.0.0.1:{}", settings.port), stream).context("WebSocket OBS")?;
    let hello = receive(&mut ws)?;
    anyhow::ensure!(hello["op"] == 0, "OBS n'a pas envoyé Hello");
    let mut identify = json!({"rpcVersion": 1, "eventSubscriptions": 205});
    if let Some(auth) = hello["d"].get("authentication") {
        let salt = auth["salt"].as_str().context("sel OBS absent")?;
        let challenge = auth["challenge"].as_str().context("défi OBS absent")?;
        identify["authentication"] = authentication(&settings.password, salt, challenge).into();
    }
    ws.send(Message::Text(json!({"op":1,"d":identify}).to_string().into()))?;
    let reply = receive(&mut ws)?;
    anyhow::ensure!(reply["op"] == 2, "Authentification OBS refusée");
    Ok(ws)
}
fn receive(ws: &mut WebSocket<TcpStream>) -> Result<Value> {
    loop {
        match ws.read()? {
            Message::Text(text) => return Ok(serde_json::from_str(&text)?),
            Message::Ping(data) => ws.send(Message::Pong(data))?,
            Message::Close(_) => bail!("OBS a fermé la connexion"),
            _ => {}
        }
    }
}
pub(crate) fn request(ws: &mut WebSocket<TcpStream>, id: &str, kind: &str, data: Value) -> Result<Value> {
    ws.send(Message::Text(json!({"op":6,"d":{"requestType":kind,"requestId":id,"requestData":data}}).to_string().into()))?;
    loop {
        let response = receive(ws)?;
        if response["op"] == 7 && response["d"]["requestId"] == id {
            anyhow::ensure!(response["d"]["requestStatus"]["result"] == true,
                "OBS a refusé la commande : {}", response["d"]["requestStatus"]["code"]);
            return Ok(response["d"]["responseData"].clone());
        }
    }
}
fn command(step: &Step) -> Result<(&'static str, Value)> {
    let (kind, args) = match step {
        Step::ObsScene { name } => ("SetCurrentProgramScene", json!({"sceneName":name})),
        Step::ObsRecord { mode } => (match mode.as_str() { "toggle"=>"ToggleRecord", "start"=>"StartRecord", "stop"=>"StopRecord", "pause"=>"PauseRecord", _=>bail!("Mode OBS invalide") },json!({})),
        Step::ObsStream { mode } => (match mode.as_str() { "toggle"=>"ToggleStream", "start"=>"StartStream", "stop"=>"StopStream", _=>bail!("Mode OBS invalide") },json!({})),
        Step::ObsVirtualCam { mode } => (match mode.as_str() { "toggle"=>"ToggleVirtualCam", "start"=>"StartVirtualCam", "stop"=>"StopVirtualCam", _=>bail!("Mode OBS invalide") },json!({})),
        Step::ObsReplay { mode } => (match mode.as_str() { "toggle"=>"ToggleReplayBuffer", "start"=>"StartReplayBuffer", "stop"=>"StopReplayBuffer", "save"=>"SaveReplayBuffer", _=>bail!("Mode OBS invalide") },json!({})),
        Step::ObsMute { input, mode } => (match mode.as_str() { "toggle"=>"ToggleInputMute", "mute"|"unmute"=>"SetInputMute", _=>bail!("Mode OBS invalide") }, json!({"inputName":input,"inputMuted": mode == "mute"})),
        Step::ObsSourceVisibility { .. } => bail!("La source OBS requiert une résolution préalable"),
        _ => bail!("Pas une commande OBS"),
    };
    Ok((kind,args))
}
struct Session { settings: (String,u16,String), ws: WebSocket<TcpStream> }
static CONNECTION: OnceLock<Mutex<Option<Session>>> = OnceLock::new();
fn with_session<T>(settings: &ObsSettings, f: impl FnOnce(&mut WebSocket<TcpStream>) -> Result<T>) -> Result<T> {
    let mut guard = CONNECTION.get_or_init(|| Mutex::new(None)).lock().unwrap_or_else(|e| e.into_inner());
    let id = (settings.host.clone(),settings.port,settings.password.clone());
    if guard.as_ref().is_none_or(|s| s.settings != id) { *guard = Some(Session { settings:id, ws:socket(settings)? }); }
    let result = f(&mut guard.as_mut().unwrap().ws);
    if result.is_err() { *guard = None; }
    result
}
fn source(ws: &mut WebSocket<TcpStream>, scene: &str, name: &str, mode: &str) -> Result<()> {
    let data = request(ws,"scene-id","GetSceneItemId",json!({"sceneName":scene,"sourceName":name}))?;
    let id = data["sceneItemId"].as_i64().context("Source OBS introuvable")?;
    let enabled = match mode {
        "show" => true, "hide" => false,
        "toggle" => !request(ws,"source-state","GetSceneItemEnabled",json!({"sceneName":scene,"sceneItemId":id}))?["sceneItemEnabled"].as_bool().context("État de la source OBS absent")?,
        _ => bail!("Mode de visibilité OBS invalide"),
    };
    request(ws,"source-set","SetSceneItemEnabled",json!({"sceneName":scene,"sceneItemId":id,"sceneItemEnabled":enabled}))?;
    Ok(())
}
pub fn execute(step: &Step, settings: &ObsSettings) -> Result<()> {
    with_session(settings, |ws| {
        if let Step::ObsSourceVisibility {scene,source: name,mode} = step { return source(ws,scene,name,mode); }
        let (kind,data) = command(step)?;
        request(ws,"deckforge-1",kind,data)?;
        Ok(())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn auth_reference() { assert_eq!(authentication("password","salt","challenge"), "zTM5ki6L2vVvBQiTG9ckH1Lh64AbnCf6XZ226UmnkIA="); }
    #[test] fn local_fake_server_authenticates_and_commands() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream,_) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            ws.send(Message::Text(json!({"op":0,"d":{"rpcVersion":1,"authentication":{"salt":"salt","challenge":"challenge"}}}).to_string().into())).unwrap();
            let identify:Value = serde_json::from_str(&ws.read().unwrap().into_text().unwrap()).unwrap();
            assert_eq!(identify["d"]["authentication"],authentication("password","salt","challenge"));
            ws.send(Message::Text(json!({"op":2,"d":{"negotiatedRpcVersion":1}}).to_string().into())).unwrap();
            let request:Value = serde_json::from_str(&ws.read().unwrap().into_text().unwrap()).unwrap();
            assert_eq!(request["d"]["requestType"],"ToggleRecord");
            ws.send(Message::Text(json!({"op":7,"d":{"requestId":"deckforge-1","requestStatus":{"result":true,"code":100},"responseData":{}}}).to_string().into())).unwrap();
        });
        execute(&Step::ObsRecord{mode:"toggle".into()}, &ObsSettings{host:"127.0.0.1".into(),port,password:"password".into()}).unwrap();
        server.join().unwrap();
    }
    #[test] fn request_serialization() {
        let (kind,data) = command(&Step::ObsMute { input:"Micro".into(), mode:"mute".into() }).unwrap();
        assert_eq!(kind,"SetInputMute"); assert_eq!(data, json!({"inputName":"Micro","inputMuted":true}));
    }
}

