use anyhow::{bail, Context, Result};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(5);

pub fn udp_send(host: &str, port: u16, text: &str) -> Result<()> {
    anyhow::ensure!(crate::model::valid_udp_host(host) && port != 0 && text.len() <= 65507, "Destination UDP invalide");
    let address = (host, port).to_socket_addrs()?.next().context("Hôte UDP introuvable")?;
    let socket = UdpSocket::bind(if address.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" })?;
    socket.set_write_timeout(Some(TIMEOUT))?;
    socket.send_to(text.as_bytes(), address)?;
    Ok(())
}

pub fn http_request(method: &str, url: &str, body: &str, content_type: &str) -> Result<String> {
    crate::model::validate_http(method, url, body, content_type)?;
    let rest = url.strip_prefix("http://").context("Adresse HTTP requise")?;
    let (authority, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    let (host, port) = authority.rsplit_once(':').map_or((authority, 80), |(h, p)| (h, p.parse::<u16>().unwrap_or(0)));
    let address = (host, port).to_socket_addrs()?.next().context("Hôte HTTP introuvable")?;
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT).context("Connexion HTTP impossible")?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    let content = if method == "GET" { "" } else { body };
    write!(stream, "{method} {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n", content.len())?;
    stream.write_all(content.as_bytes())?;
    let mut reply = Vec::new();
    stream.take(4096).read_to_end(&mut reply).context("Réponse HTTP impossible")?;
    if reply.is_empty() { bail!("Réponse HTTP vide"); }
    let first = String::from_utf8_lossy(&reply);
    Ok(format!("HTTP {} : {}{}", url, first.chars().take(1000).collect::<String>(), if reply.len() >= 4096 { " [tronqué]" } else { "" }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn udp_sends_to_local_receiver() {
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        receiver.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        udp_send("127.0.0.1", receiver.local_addr().unwrap().port(), "bonjour").unwrap();
        let mut bytes = [0; 64];
        let (n, _) = receiver.recv_from(&mut bytes).unwrap();
        assert_eq!(&bytes[..n], b"bonjour");
    }
    #[test]
    fn http_local_response_and_limits() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut client, _) = listener.accept().unwrap();
            client.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut bytes = [0; 1024];
            let mut n = 0;
            while n < bytes.len() && !bytes[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                let read = client.read(&mut bytes[n..]).unwrap();
                if read == 0 { break; }
                n += read;
            }
            assert!(String::from_utf8_lossy(&bytes[..n]).starts_with("GET /api HTTP/1.1"));
            client.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK").unwrap();
        });
        let result = http_request("GET", &format!("http://127.0.0.1:{port}/api"), "", "text/plain").unwrap();
        assert!(result.contains("200 OK"));
        server.join().unwrap();
        assert!(http_request("GET", "https://example.com", "", "text/plain").unwrap_err().to_string().contains("HTTPS non pris en charge"));
        assert!(http_request("POST", "http://127.0.0.1:80/", "", "bad\r\nHeader: evil").is_err());
    }
}
