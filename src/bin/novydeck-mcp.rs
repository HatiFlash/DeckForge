mod mcp_http;
#[path = "../token_equal.rs"]
mod token_equal;
#[path = "../mcp_protocol.rs"]
mod mcp_protocol;
use serde_json::{json, Value};
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::TcpStream,
    time::Duration,
};
fn main() {
    if let Err(e) = run() {
        eprintln!("NovyDeck MCP: {e}");
        std::process::exit(1);
    }
}
fn run() -> anyhow::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|a| a == "--http") {
        let address = args.get(1).map(String::as_str).unwrap_or("127.0.0.1:9846");
        let connection = args
            .get(2)
            .map(std::path::PathBuf::from)
            .unwrap_or(std::env::current_exe()?.with_file_name("mcp-connection.json"));
        return mcp_http::run(address, connection);
    }
    let path = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or(std::env::current_exe()?.with_file_name("mcp-connection.json"));
    let mut session = [0u8; 16];
    getrandom::fill(&mut session).map_err(|_| anyhow::anyhow!("Random generation failed"))?;
    let session = session
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut initialized = false;
    let mut cache: std::collections::HashMap<String, Result<Value, String>> =
        std::collections::HashMap::new();
    loop {
        let mut line = Vec::new();
        let n = (&mut input)
            .take(mcp_protocol::MAX_FRAME as u64 + 1)
            .read_until(b'\n', &mut line)?;
        if n == 0 {
            break;
        }
        if line.len() > mcp_protocol::MAX_FRAME {
            anyhow::bail!("MCP frame too large");
        }
        let request: Value = match serde_json::from_slice(&line) {
            Ok(v) => v,
            Err(_) => {
                writeln!(
                    output,
                    "{}",
                    mcp_protocol::error(Value::Null, -32700, "Invalid JSON")
                )?;
                output.flush()?;
                continue;
            }
        };
        let Some(id) = request.get("id").cloned() else {
            continue;
        };
        let method = request["method"].as_str().unwrap_or("");
        let response = match method {
            "initialize" => {
                initialized = true;
                mcp_protocol::success(
                    id,
                    mcp_protocol::initialize(request["params"]["protocolVersion"].as_str()),
                )
            }
            "ping" => mcp_protocol::success(id, json!({})),
            _ if !initialized => {
                mcp_protocol::error(id, -32002, "Initialize the MCP connection first")
            }
            "tools/list" => mcp_protocol::success(id, mcp_protocol::tools()),
            "tools/call" => {
                let key = format!("{}:{}", id, request["params"]);
                let result = if let Some(result) = cache.get(&key) {
                    result.clone()
                } else {
                    let result = forward(&path, &session, &id, &request["params"]);
                    if cache.len() > 128 {
                        cache.clear();
                    }
                    if result.is_ok() {
                        cache.insert(key, result.clone());
                    }
                    result
                };
                mcp_protocol::success(id, mcp_protocol::tool_result(result))
            }
            _ => mcp_protocol::error(id, -32601, "Method not found"),
        };
        writeln!(output, "{response}")?;
        output.flush()?;
    }
    Ok(())
}
fn forward(
    path: &std::path::Path,
    session: &str,
    id: &Value,
    params: &Value,
) -> Result<Value, String> {
    let c: Value = serde_json::from_slice(
        &std::fs::read(path).map_err(|_| "Ouvre NovyDeck puis active Assistant IA")?,
    )
    .map_err(|_| "Connexion locale invalide")?;
    let address = c["address"].as_str().ok_or("Adresse locale absente")?;
    let socket: std::net::SocketAddr = address.parse().map_err(|_| "Adresse locale invalide")?;
    if !socket.ip().is_loopback() {
        return Err("Le connecteur accepte uniquement localhost".into());
    }
    let mut stream = TcpStream::connect_timeout(&socket, Duration::from_secs(2))
        .map_err(|_| "NovyDeck n’est pas ouvert")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    let name = params["name"].as_str().ok_or("Nom d’outil manquant")?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    use std::hash::{Hash, Hasher};
    let mut digest = std::collections::hash_map::DefaultHasher::new();
    params.to_string().hash(&mut digest);
    let request = json!({"token":c["token"],"request_id":format!("{session}:{id}:{:x}",digest.finish()),"name":name,"arguments":args});
    writeln!(stream, "{request}").map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut bytes = Vec::new();
    (&mut reader)
        .take(mcp_protocol::MAX_FRAME as u64 + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > mcp_protocol::MAX_FRAME {
        return Err("Réponse trop volumineuse".into());
    }
    let v: Value = serde_json::from_slice(&bytes).map_err(|_| "Réponse de NovyDeck invalide")?;
    if v["ok"] == true {
        Ok(v["result"].clone())
    } else {
        Err(v["error"].as_str().unwrap_or("Erreur NovyDeck").into())
    }
}
