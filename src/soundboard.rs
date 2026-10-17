//! MCI is asynchronous: playback is never awaited by the macro engine.
use anyhow::{bail, ensure, Result};
use std::sync::{Mutex, OnceLock};
use windows_sys::Win32::Media::Multimedia::mciSendStringW;

#[derive(Default)]
struct Players { next: u64, active: Vec<String> }
static PLAYERS: OnceLock<Mutex<Players>> = OnceLock::new();
fn players() -> &'static Mutex<Players> { PLAYERS.get_or_init(|| Mutex::new(Players::default())) }
fn command(text: &str) -> Result<String> {
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let mut out = [0u16; 128];
    let result = unsafe { mciSendStringW(wide.as_ptr(), out.as_mut_ptr(), out.len() as u32, std::ptr::null_mut()) };
    ensure!(result == 0, "Windows MCI a refusé la commande audio (code {result})");
    let end = out.iter().position(|c| *c == 0).unwrap_or(out.len());
    Ok(String::from_utf16_lossy(&out[..end]))
}
fn close(alias: &str) { let _ = command(&format!("close {alias}")); }
pub fn play(path: &str, volume: u8) -> Result<()> {
    ensure!(volume <= 100, "Volume audio invalide");
    let p = std::path::Path::new(path);
    ensure!(p.is_file(), "Fichier son introuvable : {path}");
    let absolute = p.canonicalize()?;
    let name = absolute.to_string_lossy();
    ensure!(!name.contains('"') && !name.chars().any(char::is_control), "Chemin audio invalide");
    let mut state = players().lock().unwrap_or_else(|e| e.into_inner());
    state.active.retain(|alias| {
        if command(&format!("status {alias} mode")).is_ok_and(|mode| mode == "playing" || mode == "paused") { true }
        else { close(alias); false }
    });
    if state.active.len() >= 8 { bail!("8 sons simultanés maximum"); }
    state.next = state.next.wrapping_add(1);
    let alias = format!("dfsound{}", state.next);
    command(&format!("open \"{name}\" type mpegvideo alias {alias}"))?;
    if let Err(error) = command(&format!("setaudio {alias} volume to {}", u32::from(volume) * 10))
        .and_then(|_| command(&format!("play {alias}"))) {
        close(&alias);
        return Err(error);
    }
    state.active.push(alias);
    Ok(())
}
pub fn stop() {
    let mut state = players().lock().unwrap_or_else(|e| e.into_inner());
    for alias in state.active.drain(..) { close(&alias); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_sound_never_opens_a_player() {
        assert!(play("Z:\\\\deckforge-no-such-sound.wav", 50).is_err());
        assert!(players().lock().unwrap().active.is_empty());
        stop();
    }
}
