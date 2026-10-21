use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    time::Duration,
};
pub struct Call {
    pub name: String,
    pub arguments: Value,
    pub reply: SyncSender<Result<Value, String>>,
    pub id: String,
}
pub struct Server {
    pub requests: Receiver<Call>,
    enabled: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    path: PathBuf,
}
impl Server {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?.to_string();
        let mut random = [0u8; 32];
        getrandom::fill(&mut random)
            .map_err(|_| anyhow::anyhow!("Génération de la clé locale impossible"))?;
        let token = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let descriptor = json!({"address":address,"token":token,"version":1});
        std::fs::write(&path, serde_json::to_vec(&descriptor)?)?;
        let enabled = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, requests) = mpsc::sync_channel::<Call>(16);
        let on = enabled.clone();
        let stopped = stop.clone();
        std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // Windows : le socket accepté hérite du mode non bloquant ; le délai de lecture de handle() doit s'appliquer.
                        let _ = stream.set_nonblocking(false);
                        let result = handle(&mut stream, &token, &on, &tx);
                        let response = match result {
                            Ok(v) => json!({"ok":true,"result":v}),
                            Err(e) => json!({"ok":false,"error":e}),
                        };
                        let _ = writeln!(stream, "{response}");
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            requests,
            enabled,
            stop,
            path,
        })
    }
    pub fn set_enabled(&self, value: bool) {
        self.enabled.store(value, Ordering::SeqCst);
    }
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.enabled.store(false, Ordering::SeqCst);
        self.stop.store(true, Ordering::SeqCst);
        let _ = std::fs::remove_file(&self.path);
    }
}
fn handle(
    stream: &mut TcpStream,
    token: &str,
    on: &AtomicBool,
    tx: &mpsc::SyncSender<Call>,
) -> Result<Value, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut bytes = Vec::new();
    (&mut reader)
        .take(crate::mcp_protocol::MAX_FRAME as u64 + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > crate::mcp_protocol::MAX_FRAME {
        return Err("Requête trop volumineuse".into());
    }
    let v: Value = serde_json::from_slice(&bytes).map_err(|_| "Requête locale invalide")?;
    if !v["token"].as_str().is_some_and(|value| crate::token_equal::equal(value, token)) {
        return Err("Accès local refusé".into());
    }
    if !on.load(Ordering::SeqCst) {
        return Err("Active Assistant IA dans NovyDeck pour autoriser ce client MCP".into());
    }
    let name = v["name"]
        .as_str()
        .ok_or("Nom d’outil manquant")?
        .to_string();
    let id = v["request_id"]
        .as_str()
        .ok_or("Identifiant manquant")?
        .to_string();
    let arguments = v.get("arguments").cloned().unwrap_or(json!({}));
    let (reply, rx) = mpsc::sync_channel(1);
    tx.try_send(Call {
        name,
        arguments,
        reply,
        id,
    })
    .map_err(|_| "NovyDeck est occupé")?;
    rx.recv_timeout(Duration::from_secs(3)).map_err(|_| {
        "NovyDeck ne répond pas. Vérifie l’application avant de réessayer.".to_string()
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(path: &std::path::Path, override_token: Option<&str>) -> Value {
        let c: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut stream = TcpStream::connect(c["address"].as_str().unwrap()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let v = json!({"token":override_token.unwrap_or(c["token"].as_str().unwrap()),"name":"deckforge_get_state","request_id":"test-session:1","arguments":{}});
        writeln!(stream, "{v}").unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
    #[test]
    fn disabled_and_unauthenticated_clients_cannot_reach_app() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("connection.json");
        let server = Server::new(path.clone()).unwrap();
        assert_eq!(request(&path, None)["ok"], false);
        server.set_enabled(true);
        assert_eq!(request(&path, Some("wrong-token"))["ok"], false);
        assert!(server.requests.try_recv().is_err());
        drop(server);
        assert!(!path.exists());
    }
    #[test]
    fn enabled_authenticated_call_round_trips_without_mutating_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("connection.json");
        let server = Server::new(path.clone()).unwrap();
        server.set_enabled(true);
        let client = std::thread::spawn(move || request(&path, None));
        let call = server
            .requests
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert_eq!(call.name, "deckforge_get_state");
        call.reply.send(Ok(json!({"connected":true}))).unwrap();
        let response = client.join().unwrap();
        assert_eq!(response["result"]["connected"], true);
    }
}
