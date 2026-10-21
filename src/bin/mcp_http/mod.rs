use super::{forward, mcp_protocol, token_equal};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
    sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}},
};
struct Session {
    last: Instant,
    cache: HashMap<String, Result<Value, String>>,
}
pub fn run(address: &str, connection: PathBuf) -> anyhow::Result<()> {
    let address: std::net::SocketAddr = address.parse()?;
    anyhow::ensure!(
        address.ip().is_loopback(),
        "HTTP MCP must bind to localhost"
    );
    let token_path = connection.with_file_name("mcp-http-token.txt");
    let token = if let Ok(t) = std::fs::read_to_string(&token_path) {
        let t = t.trim().to_string();
        anyhow::ensure!(
            t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid HTTP token file"
        );
        t
    } else {
        let t = random()?;
        std::fs::write(&token_path, &t)?;
        t
    };
    let listener = TcpListener::bind(address)?;
    eprintln!(
        "NovyDeck MCP HTTP: http://{address}/mcp (Bearer token in {})",
        token_path.display()
    );
    let sessions = Arc::new(Mutex::new(HashMap::<String, Session>::new()));
    let active = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        let mut stream = stream?;
        if active.fetch_add(1, Ordering::AcqRel) >= 8 {
            active.fetch_sub(1, Ordering::AcqRel);
            stream.set_write_timeout(Some(Duration::from_secs(3)))?;
            let _ = reply(&mut stream, 503, &[], "{\"error\":\"Server busy\"}");
            continue;
        }
        let sessions = sessions.clone();
        let active = active.clone();
        let token = token.clone();
        let connection = connection.clone();
        std::thread::spawn(move || {
            struct Slot(Arc<AtomicUsize>);
            impl Drop for Slot {
                fn drop(&mut self) { self.0.fetch_sub(1, Ordering::AcqRel); }
            }
            let _slot = Slot(active);
            let response = handle(&mut stream, &token, &connection, address.port(), &sessions);
            if let Err(e) = response {
                let _ = reply(&mut stream, 400, &[], &json!({"error":e.to_string()}).to_string());
            }
        });
    }
    Ok(())
}
fn random() -> anyhow::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| anyhow::anyhow!("Random generation failed"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn reply(
    stream: &mut TcpStream,
    status: u16,
    headers: &[(&str, String)],
    body: &str,
) -> anyhow::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        204 => "No Content",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        _ => "Bad Request",
    };
    write!(stream,"HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n",body.len())?;
    for (name, value) in headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    write!(stream, "\r\n{body}")?;
    Ok(())
}
fn handle(
    stream: &mut TcpStream,
    token: &str,
    connection: &Path,
    port: u16,
    sessions: &Arc<Mutex<HashMap<String, Session>>>,
) -> anyhow::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    (&mut reader).take(16385).read_line(&mut line)?;
    let first = line.split_whitespace().collect::<Vec<_>>();
    anyhow::ensure!(first.len() == 3, "Invalid request line");
    let method = first[0].to_string();
    let path = first[1].to_string();
    let mut headers = HashMap::new();
    let mut total = line.len();
    loop {
        line.clear();
        let n = (&mut reader).take(16385).read_line(&mut line)?;
        total += n;
        anyhow::ensure!(n > 0 && total <= 16384, "Headers too large");
        if line == "\r\n" || line == "\n" {
            break;
        }
        let (k, v) = line
            .trim_end()
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("Invalid header"))?;
        headers.insert(k.trim().to_lowercase(), v.trim().to_string());
    }
    let hosts = [
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("[::1]:{port}"),
    ];
    if !headers.get("host").is_some_and(|h| hosts.contains(h)) {
        return reply(stream, 403, &[], "{\"error\":\"Host not allowed\"}");
    }
    if let Some(origin) = headers.get("origin") {
        if !hosts.iter().any(|h| origin == &format!("http://{h}")) {
            return reply(stream, 403, &[], "{\"error\":\"Origin not allowed\"}");
        }
    }
    if !headers
        .get("authorization")
        .is_some_and(|h| token_equal::equal(h, &format!("Bearer {token}")))
    {
        return reply(
            stream,
            401,
            &[("WWW-Authenticate", "Bearer".into())],
            "{\"error\":\"Bearer token required\"}",
        );
    }
    if path != "/mcp" {
        return reply(stream, 404, &[], "{}");
    }
    if method == "GET" {
        return reply(stream, 405, &[("Allow", "POST, DELETE".into())], "{}");
    }
    let mut sessions = sessions.lock().unwrap_or_else(|e| e.into_inner());
    sessions.retain(|_, s| s.last.elapsed() < Duration::from_secs(900));
    if method == "DELETE" {
        if let Some(id) = headers.get("mcp-session-id") {
            sessions.remove(id);
        }
        return reply(stream, 204, &[], "");
    }
    if method != "POST" {
        return reply(stream, 405, &[("Allow", "POST, DELETE".into())], "{}");
    }
    if let Some(version) = headers.get("mcp-protocol-version") {
        if !["2024-11-05", "2025-03-26", mcp_protocol::VERSION].contains(&version.as_str()) {
            return reply(
                stream,
                400,
                &[],
                "{\"error\":\"Unsupported protocol version\"}",
            );
        }
    }
    anyhow::ensure!(
        !headers.contains_key("transfer-encoding"),
        "Chunked requests are not supported"
    );
    let length = headers
        .get("content-length")
        .ok_or_else(|| anyhow::anyhow!("Content-Length required"))?
        .parse::<usize>()?;
    if length > mcp_protocol::MAX_FRAME {
        return reply(stream, 413, &[], "{}");
    }
    let mut bytes = vec![0u8; length];
    reader.read_exact(&mut bytes)?;
    let request: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            return reply(
                stream,
                400,
                &[],
                &mcp_protocol::error(Value::Null, -32700, "Invalid JSON").to_string(),
            )
        }
    };
    if request["jsonrpc"] != "2.0" {
        return reply(
            stream,
            400,
            &[],
            &mcp_protocol::error(Value::Null, -32600, "Invalid JSON-RPC request").to_string(),
        );
    }
    let method = request["method"].as_str().unwrap_or("");
    if method == "initialize" {
        anyhow::ensure!(sessions.len() < 32, "Too many MCP sessions");
        let id = request
            .get("id")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Initialize requires id"))?;
        let session = random()?;
        sessions.insert(
            session.clone(),
            Session {
                last: Instant::now(),
                cache: HashMap::new(),
            },
        );
        return reply(
            stream,
            200,
            &[("Mcp-Session-Id", session)],
            &mcp_protocol::success(
                id,
                mcp_protocol::initialize(request["params"]["protocolVersion"].as_str()),
            )
            .to_string(),
        );
    }
    let session_id = headers
        .get("mcp-session-id")
        .ok_or_else(|| anyhow::anyhow!("Mcp-Session-Id required"))?;
    let Some(session) = sessions.get_mut(session_id) else {
        return reply(stream, 404, &[], "{\"error\":\"Session expired\"}");
    };
    session.last = Instant::now();
    let Some(id) = request.get("id").cloned() else {
        return reply(stream, 202, &[], "");
    };
    let response = match method {
        "ping" => mcp_protocol::success(id, json!({})),
        "tools/list" => mcp_protocol::success(id, mcp_protocol::tools()),
        "tools/call" => {
            let key = format!("{}:{}", id, request["params"]);
            let result = if let Some(v) = session.cache.get(&key) {
                v.clone()
            } else {
                let result = forward(connection, session_id, &id, &request["params"]);
                if session.cache.len() > 128 {
                    session.cache.clear();
                }
                if result.is_ok() {
                    session.cache.insert(key, result.clone());
                }
                result
            };
            mcp_protocol::success(id, mcp_protocol::tool_result(result))
        }
        _ => mcp_protocol::error(id, -32601, "Method not found"),
    };
    reply(stream, 200, &[], &response.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_slot_count_recovers_after_busy_request() {
        let active = AtomicUsize::new(8);
        assert!(active.fetch_add(1, Ordering::AcqRel) >= 8);
        active.fetch_sub(1, Ordering::AcqRel);
        assert_eq!(active.load(Ordering::Acquire), 8);
    }
    #[test]
    fn bearer_comparison_checks_full_value() {
        assert!(token_equal::equal("Bearer abc", "Bearer abc"));
        assert!(!token_equal::equal("Bearer abc", "Bearer abd"));
        assert!(!token_equal::equal("Bearer abc", "abc"));
    }
}
