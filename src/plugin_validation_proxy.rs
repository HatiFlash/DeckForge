//! Proxy HTTP éphémère de banc d'essai, non branché à la validation HTML.
//! Ce proxy ne confine ni le navigateur ni l'OS (DNS, UDP, extensions, accès direct restent possibles).
use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const LIMIT: usize = 8192;
const MAX_CONNECTIONS: usize = 16;
const TIMEOUT: Duration = Duration::from_secs(2);
const LIFETIME: Duration = Duration::from_secs(15);

pub struct Proxy {
    pub port: u16,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    pub log: Arc<Mutex<Vec<String>>>,
}

fn reject(log: &Arc<Mutex<Vec<String>>>, category: &'static str, endpoint: &str) {
    // endpoint est exclusivement une constante construite depuis la configuration fiable.
    if let Ok(mut entries) = log.lock() {
        if entries.len() < 64 {
            entries.push(format!("{category} {endpoint}"));
        }
    }
}

fn headers(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut b = [0u8; 1];
    let started = Instant::now();
    loop {
        if started.elapsed() >= TIMEOUT || bytes.len() >= LIMIT {
            return Err(io::Error::other("header_limit_or_timeout"));
        }
        stream.read_exact(&mut b)?;
        bytes.push(b[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            return Ok(bytes);
        }
    }
}

fn parse(request: &[u8], resource: u16, ws: u16) -> Option<(u16, Vec<u8>, bool)> {
    let s = std::str::from_utf8(request).ok()?;
    if !s.ends_with("\r\n\r\n")
        || s[..s.len() - 4]
            .split("\r\n")
            .any(|line| line.contains('\r') || line.contains('\n'))
    {
        return None;
    }
    let mut lines = s[..s.len() - 4].split("\r\n");
    let first = lines.next()?;
    let mut parts = first.split(' ');
    let method = parts.next()?;
    let url = parts.next()?;
    if parts.next()? != "HTTP/1.1" || parts.next().is_some() {
        return None;
    }
    let (upgrade, port, path) = if let Some(rest) = url.strip_prefix("http://127.0.0.1:") {
        let (p, path) = rest.split_once('/')?;
        (false, p.parse::<u16>().ok()?, format!("/{path}"))
    } else if let Some(rest) = url.strip_prefix("ws://127.0.0.1:") {
        let (p, path) = rest.split_once('/')?;
        (true, p.parse::<u16>().ok()?, format!("/{path}"))
    } else {
        return None;
    };
    if path.contains('#') || path.bytes().any(|b| b < 0x21 || b == 0x7f) {
        return None;
    }
    if (upgrade && port != ws)
        || (!upgrade && port != resource)
        || (upgrade && method != "GET")
        || (!upgrade && method != "GET" && method != "HEAD")
    {
        return None;
    }
    let expected_host = format!("127.0.0.1:{port}");
    let mut host = false;
    let mut conn = false;
    let mut up = false;
    let mut key = false;
    let mut version = false;
    let mut ws_key = "";
    for line in lines {
        let (name, value) = line.split_once(':')?;
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.bytes().any(|b| b < 32 && b != b' ' || b == 127)
            || value.contains(':')
                && name.eq_ignore_ascii_case("host")
                && value.trim() != expected_host
        {
            return None;
        }
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "host" if !host && value == expected_host => host = true,
            "connection" if upgrade && !conn && value.eq_ignore_ascii_case("upgrade") => {
                conn = true
            }
            "upgrade" if upgrade && !up && value.eq_ignore_ascii_case("websocket") => up = true,
            "sec-websocket-key"
                if upgrade
                    && !key
                    && value.len() == 24
                    && value.bytes().all(|b| {
                        b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'='
                    }) =>
            {
                key = true;
                ws_key = value;
            }
            "sec-websocket-version" if upgrade && !version && value == "13" => version = true,
            "accept" | "accept-language" | "user-agent" | "cache-control" if !upgrade => {}
            _ => return None,
        }
    }
    if !host || (upgrade && !(conn && up && key && version)) {
        return None;
    }
    let outbound = if upgrade {
        format!("GET {path} HTTP/1.1\r\nHost: {expected_host}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Key: {ws_key}\r\nSec-WebSocket-Version: 13\r\n\r\n")
    } else {
        format!("{method} {path} HTTP/1.1\r\nHost: {expected_host}\r\nConnection: close\r\n\r\n")
    };
    Some((port, outbound.into_bytes(), upgrade))
}

fn valid_response(bytes: &[u8], websocket: bool) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    if !text.ends_with("\r\n\r\n")
        || text[..text.len() - 4]
            .split("\r\n")
            .any(|s| s.contains('\r') || s.contains('\n'))
    {
        return false;
    }
    let mut lines = text[..text.len() - 4].split("\r\n");
    let expected = if websocket {
        "HTTP/1.1 101 "
    } else {
        "HTTP/1.1 200 "
    };
    if !lines.next().is_some_and(|line| {
        line.starts_with(expected) && line.len() < 96 && line.bytes().all(|b| b >= 32 && b < 127)
    }) {
        return false;
    }
    let mut upgrade = false;
    let mut content_length = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return false;
        };
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.bytes().any(|b| b < 32 || b == 127)
        {
            return false;
        }
        match name.to_ascii_lowercase().as_str() {
            "location" | "proxy-authenticate" | "transfer-encoding" => return false,
            "content-length" => {
                if content_length || value.trim().parse::<usize>().is_err() {
                    return false;
                }
                content_length = true;
            }
            "upgrade" => {
                if upgrade || !websocket || !value.trim().eq_ignore_ascii_case("websocket") {
                    return false;
                }
                upgrade = true;
            }
            _ => {}
        }
    }
    !websocket || upgrade
}

fn serve(mut client: TcpStream, resource: u16, ws: u16, log: &Arc<Mutex<Vec<String>>>) {
    let _ = client.set_nonblocking(false);
    let _ = client.set_read_timeout(Some(TIMEOUT));
    let _ = client.set_write_timeout(Some(TIMEOUT));
    let allowed = format!("127.0.0.1:{resource}|127.0.0.1:{ws}");
    let result = (|| -> io::Result<()> {
        let req = headers(&mut client)?;
        let Some((port, outbound, websocket)) = parse(&req, resource, ws) else {
            reject(log, "denied", &allowed);
            client.write_all(
                b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )?;
            return Ok(());
        };
        let mut upstream =
            TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().unwrap(), TIMEOUT)?;
        upstream.set_read_timeout(Some(TIMEOUT))?;
        upstream.set_write_timeout(Some(TIMEOUT))?;
        upstream.write_all(&outbound)?;
        let response = headers(&mut upstream)?;
        // Aucun statut de redirection ni en-tête malformed n'atteint le client.
        if !valid_response(&response, websocket) {
            reject(log, "upstream_denied", &allowed);
            client.write_all(
                b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )?;
            return Ok(());
        }
        reject(log, "allowed", &allowed);
        client.write_all(&response)?;
        if websocket {
            let mut from_client = client.try_clone()?;
            let mut to_upstream = upstream.try_clone()?;
            let sender = thread::spawn(move || {
                let _ = bounded_copy(&mut from_client, &mut to_upstream);
            });
            let _ = bounded_copy(&mut upstream, &mut client);
            let _ = sender.join();
        } else {
            let _ = bounded_copy(&mut upstream, &mut client);
        }
        Ok(())
    })();
    if result.is_err() {
        reject(log, "io_error", &allowed);
    }
}

fn bounded_copy(from: &mut TcpStream, to: &mut TcpStream) -> io::Result<()> {
    let mut total = 0usize;
    let mut buf = [0u8; 4096];
    let start = Instant::now();
    while total < 1024 * 1024 && start.elapsed() < LIFETIME {
        match from.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                to.write_all(&buf[..n])?;
            }
            Err(e)
                if e.kind() == io::ErrorKind::TimedOut || e.kind() == io::ErrorKind::WouldBlock =>
            {
                break
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

impl Proxy {
    pub fn start(resource: u16, ws: u16) -> io::Result<Self> {
        if resource == 0 || ws == 0 || resource == ws {
            return Err(io::Error::other("invalid_endpoints"));
        }
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let log = Arc::new(Mutex::new(Vec::new()));
        let events = log.clone();
        let worker = thread::spawn(move || {
            let start = Instant::now();
            let mut count = 0;
            while !flag.load(Ordering::Relaxed) && start.elapsed() < LIFETIME {
                match listener.accept() {
                    Ok((mut stream, _)) if count >= MAX_CONNECTIONS => {
                        let _ = stream.write_all(
                            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n",
                        );
                        reject(&events, "quota", "loopback");
                    }
                    Ok((stream, _)) => {
                        count += 1;
                        serve(stream, resource, ws, &events);
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            port,
            stop,
            worker: Some(worker),
            log,
        })
    }
}
impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    fn canary(reply: &'static [u8]) -> (u16, Arc<AtomicUsize>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let count = hits.clone();
        let job = thread::spawn(move || {
            let until = Instant::now() + Duration::from_millis(650);
            while Instant::now() < until {
                if let Ok((mut s, _)) = listener.accept() {
                    count.fetch_add(1, Ordering::SeqCst);
                    s.set_read_timeout(Some(TIMEOUT)).unwrap();
                    let _ = headers(&mut s);
                    let _ = s.write_all(reply);
                } else {
                    thread::sleep(Duration::from_millis(5));
                }
            }
        });
        (port, hits, job)
    }
    fn request(port: u16, text: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(TIMEOUT)).unwrap();
        s.write_all(text.as_bytes()).unwrap();
        let mut out = String::new();
        let _ = s.read_to_string(&mut out);
        out
    }
    #[test]
    fn allow_and_deny_canaries_without_leaking_secrets() {
        let (resource, rh, rj) = canary(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        let (ws, wh, wj) = canary(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n");
        let (forbidden, fh, fj) = canary(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        let proxy = Proxy::start(resource, ws).unwrap();
        assert!(request(proxy.port, &format!("GET http://127.0.0.1:{resource}/SECRET?token=SECRET HTTP/1.1\r\nHost: 127.0.0.1:{resource}\r\n\r\n")).starts_with("HTTP/1.1 200"));
        assert!(request(
            proxy.port,
            &format!(
                "HEAD http://127.0.0.1:{resource}/ HTTP/1.1\r\nHost: 127.0.0.1:{resource}\r\n\r\n"
            )
        )
        .starts_with("HTTP/1.1 200"));
        assert!(request(proxy.port, &format!("GET ws://127.0.0.1:{ws}/SECRET HTTP/1.1\r\nHost: 127.0.0.1:{ws}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n")).starts_with("HTTP/1.1 101"));
        for raw in [
            format!("GET http://127.0.0.1:{forbidden}/SECRET?SECRET HTTP/1.1\r\nHost: 127.0.0.1:{forbidden}\r\n\r\n"),
            format!("CONNECT 127.0.0.1:{forbidden} HTTP/1.1\r\nHost: 127.0.0.1:{forbidden}\r\n\r\n"),
            format!("GET ws://127.0.0.1:{forbidden}/SECRET HTTP/1.1\r\nHost: 127.0.0.1:{forbidden}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"),
            format!("POST http://127.0.0.1:{resource}/ HTTP/1.1\r\nHost: 127.0.0.1:{resource}\r\n\r\n"),
            format!("GET http://127.0.0.1:{resource}/ HTTP/1.1\r\nHost: 127.0.0.1:{resource}\r\nBad: SECRET\r\n\r\n"),
        ] { assert!(request(proxy.port, &raw).starts_with("HTTP/1.1 403")); }
        assert_eq!(rh.load(Ordering::SeqCst), 2);
        assert_eq!(wh.load(Ordering::SeqCst), 1);
        assert!(!proxy.log.lock().unwrap().join(" ").contains("SECRET"));
        drop(proxy);
        rj.join().unwrap();
        wj.join().unwrap();
        fj.join().unwrap();
        assert_eq!(fh.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn malformed_upstream_headers_fail_closed() {
        for header in [
            b"HTTP/1.1 200 OK\r\nLocation: http://127.0.0.1:9999/SECRET\r\n\r\n".as_slice(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nBroken\r\n\r\n",
        ] {
            assert!(!valid_response(header, false));
        }
        assert!(!valid_response(b"HTTP/1.1 302 Found\r\n\r\n", false));
    }
    #[test]
    fn default_deny_redirect_and_malformed() {
        let (resource, _, rj) = canary(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9999/SECRET\r\nContent-Length: 0\r\n\r\n");
        let (ws, _, wj) = canary(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\r\n");
        let proxy = Proxy::start(resource, ws).unwrap();
        assert!(request(
            proxy.port,
            &format!(
                "GET http://127.0.0.1:{resource}/ HTTP/1.1\r\nHost: 127.0.0.1:{resource}\r\n\r\n"
            )
        )
        .starts_with("HTTP/1.1 502"));
        assert!(request(proxy.port, &format!("GET http://127.0.0.1:{resource}/ HTTP/1.1\r\nHost: 127.0.0.1:{resource}\r\nHost: 127.0.0.1:{resource}\r\n\r\n")).starts_with("HTTP/1.1 403"));
        assert!(!proxy.log.lock().unwrap().join(" ").contains("SECRET"));
        drop(proxy);
        rj.join().unwrap();
        wj.join().unwrap();
    }
}
