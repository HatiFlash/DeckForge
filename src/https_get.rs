//! Téléchargements HTTPS bornés pour la bibliothèque communautaire (aucune requête implicite).
#![cfg(windows)]
use std::{ffi::c_void, ptr};
use anyhow::{Context, Result};
use windows_sys::Win32::Networking::WinHttp::*;

const REDIRECTS: usize = 5;
fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }
fn checked_url(raw: &str) -> Result<url::Url> {
    let url = url::Url::parse(raw).context("Adresse HTTPS invalide")?;
    anyhow::ensure!(url.scheme() == "https" && url.host_str().is_some() && url.username().is_empty() && url.password().is_none(), "Seules les adresses HTTPS sans identifiants sont autorisées");
    Ok(url)
}
struct Handle(*mut c_void);
impl Handle {
    fn new(raw: *mut c_void) -> Result<Self> {
        anyhow::ensure!(!raw.is_null(), "Connexion WinHTTP impossible : {}", std::io::Error::last_os_error());
        Ok(Self(raw))
    }
}
impl Drop for Handle { fn drop(&mut self) { unsafe { WinHttpCloseHandle(self.0); } } }
fn ok(value: i32, step: &str) -> Result<()> {
    anyhow::ensure!(value != 0, "{step} : {}", std::io::Error::last_os_error());
    Ok(())
}
fn header(request: &Handle, kind: u32) -> Result<Option<String>> {
    let mut len = 0u32;
    // Premier appel : longueur en octets retournée par WinHTTP, y compris le NUL UTF-16.
    unsafe { WinHttpQueryHeaders(request.0, kind, ptr::null(), ptr::null_mut(), &mut len, ptr::null_mut()); }
    if len == 0 { return Ok(None); }
    anyhow::ensure!(len <= 16 * 1024 && len % 2 == 0, "En-tête HTTP trop long ou invalide");
    let mut buffer = vec![0u16; len as usize / 2];
    ok(unsafe { WinHttpQueryHeaders(request.0, kind, ptr::null(), buffer.as_mut_ptr().cast(), &mut len, ptr::null_mut()) }, "Lecture d'en-tête HTTP impossible")?;
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Ok(Some(String::from_utf16(&buffer[..end]).context("En-tête HTTP invalide")?))
}

/// GET HTTPS uniquement ; redirections contrôlées pour éviter tout repli en HTTP.
pub fn get(raw: &str, max_bytes: usize) -> Result<Vec<u8>> {
    anyhow::ensure!(max_bytes > 0 && max_bytes <= 50 * 1024 * 1024, "Limite de téléchargement invalide");
    let mut url = checked_url(raw)?;
    let agent = wide(&format!("NovyDeck/{}", env!("CARGO_PKG_VERSION")));
    let session = Handle::new(unsafe { WinHttpOpen(agent.as_ptr(), WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, ptr::null(), ptr::null(), 0) })?;
    ok(unsafe { WinHttpSetTimeouts(session.0, 15_000, 15_000, 15_000, 15_000) }, "Délais WinHTTP impossibles")?;
    for redirect in 0..=REDIRECTS {
        let host = wide(url.host_str().context("Hôte HTTPS absent")?);
        let connect = Handle::new(unsafe { WinHttpConnect(session.0, host.as_ptr(), url.port_or_known_default().unwrap_or(443), 0) })?;
        let mut resource = url.path().to_owned();
        if let Some(query) = url.query() { resource.push('?'); resource.push_str(query); }
        let resource = wide(&resource);
        let verb = wide("GET");
        let request = Handle::new(unsafe { WinHttpOpenRequest(connect.0, verb.as_ptr(), resource.as_ptr(), ptr::null(), ptr::null(), ptr::null(), WINHTTP_FLAG_SECURE) })?;
        let policy = WINHTTP_OPTION_REDIRECT_POLICY_NEVER;
        ok(unsafe { WinHttpSetOption(request.0, WINHTTP_OPTION_REDIRECT_POLICY, (&policy as *const u32).cast(), std::mem::size_of_val(&policy) as u32) }, "Politique de redirection impossible")?;
        ok(unsafe { WinHttpSendRequest(request.0, ptr::null(), 0, ptr::null(), 0, 0, 0) }, "Requête HTTPS impossible")?;
        ok(unsafe { WinHttpReceiveResponse(request.0, ptr::null_mut()) }, "Réponse HTTPS impossible")?;
        let status: u16 = header(&request, WINHTTP_QUERY_STATUS_CODE)?.context("Code HTTP absent")?.parse().context("Code HTTP invalide")?;
        if matches!(status, 301 | 302 | 303 | 307 | 308) {
            anyhow::ensure!(redirect < REDIRECTS, "Trop de redirections HTTPS");
            let location = header(&request, WINHTTP_QUERY_LOCATION)?.context("Redirection sans destination")?;
            url = checked_url(url.join(&location)?.as_str())?;
            continue;
        }
        anyhow::ensure!(status == 200, "Téléchargement HTTPS refusé (HTTP {status})");
        if let Some(length) = header(&request, WINHTTP_QUERY_CONTENT_LENGTH)? {
            anyhow::ensure!(length.parse::<u64>().context("Taille HTTP invalide")? <= max_bytes as u64, "Téléchargement trop volumineux");
        }
        let mut body = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        loop {
            let mut read = 0u32;
            ok(unsafe { WinHttpReadData(request.0, chunk.as_mut_ptr().cast(), chunk.len() as u32, &mut read) }, "Lecture HTTPS impossible")?;
            if read == 0 { return Ok(body); }
            anyhow::ensure!((read as usize) <= max_bytes.saturating_sub(body.len()), "Téléchargement trop volumineux");
            body.extend_from_slice(&chunk[..read as usize]);
        }
    }
    anyhow::bail!("Trop de redirections HTTPS")
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn rejects_non_https_and_credentials() {
        for address in ["http://example.org/a", "https://user:pass@example.org/a", "file:///x"] { assert!(checked_url(address).is_err()); }
        assert!(checked_url("https://github.com/a").is_ok());
    }
}
