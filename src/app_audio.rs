//! Volume des sessions d'une application, jamais du volume principal d'un périphérique.
use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashSet;
use windows::{core::Interface, Win32::{Media::Audio::*, System::Com::*}};
use windows_sys::Win32::{Foundation::CloseHandle, System::Diagnostics::ToolHelp::*, UI::WindowsAndMessaging::*};

#[derive(Clone)]
struct Process { pid: u32, parent: u32, name: String }

fn is_own_process(name: &str) -> bool {
    name.eq_ignore_ascii_case("novydeck.exe") || name.eq_ignore_ascii_case("deckforge.exe")
}
#[derive(Clone)]
pub struct Target { pids: HashSet<u32>, label: String }

fn processes() -> Result<Vec<Process>> {
    unsafe {
        let handle = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if handle.is_null() || handle as isize == -1 { bail!("Impossible de consulter les processus Windows"); }
        struct Guard(windows_sys::Win32::Foundation::HANDLE);
        impl Drop for Guard { fn drop(&mut self) { unsafe { CloseHandle(self.0); } } }
        let _guard = Guard(handle);
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut list = Vec::new();
        if Process32FirstW(handle, &mut entry) == 0 { bail!("Impossible de lire les processus Windows"); }
        loop {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            list.push(Process { pid: entry.th32ProcessID, parent: entry.th32ParentProcessID, name: String::from_utf16_lossy(&entry.szExeFile[..len]) });
            if Process32NextW(handle, &mut entry) == 0 { break; }
        }
        Ok(list)
    }
}

pub fn capture(target: &str, include_children: bool) -> Result<Target> {
    let list = processes()?;
    let roots: HashSet<u32> = if target == "foreground" {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid); }
        if pid == 0 { bail!("Aucune application au premier plan. Reviens sur l'application cible."); }
        let name = list.iter().find(|p| p.pid == pid).map(|p| p.name.as_str()).context("L'application cible s'est fermée")?;
        if pid == std::process::id() || is_own_process(name) {
            bail!("NovyDeck est au premier plan. Reviens sur l'application cible.");
        }
        HashSet::from([pid])
    } else {
        let found: HashSet<_> = list.iter().filter(|p| p.name.eq_ignore_ascii_case(target) && p.pid != std::process::id()).map(|p| p.pid).collect();
        if found.is_empty() { bail!("Application introuvable : {target}"); }
        found
    };
    let label = if target == "foreground" {
        list.iter().find(|p| roots.contains(&p.pid)).map(|p| p.name.clone()).unwrap_or_default()
    } else { target.to_owned() };
    let mut pids = roots;
    if include_children {
        loop {
            let count = pids.len();
            for p in &list { if pids.contains(&p.parent) { pids.insert(p.pid); } }
            if count == pids.len() { break; }
        }
    }
    // Explicit targets must never select DeckForge itself, even as a child.
    pids.remove(&std::process::id());
    pids.retain(|pid| !list.iter().any(|p| p.pid == *pid && is_own_process(&p.name)));
    if pids.is_empty() { bail!("NovyDeck n'est pas une application cible audio"); }
    Ok(Target { pids, label })
}

pub fn adjust(current: f32, action: &str, step: u8, level: u8) -> f32 {
    match action {
        "up" => (current + f32::from(step) / 100.0).clamp(0.0, 1.0),
        "down" => (current - f32::from(step) / 100.0).clamp(0.0, 1.0),
        "set" => f32::from(level) / 100.0,
        _ => current,
    }
}
pub fn should_mute(states: &[bool]) -> bool { states.iter().any(|&muted| !muted) }

pub fn apply(target: &Target, action: &str, step: u8, level: u8) -> Result<String> {
    // COM apartment lives on the engine worker thread; no interface crosses threads.
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok().context("Initialisation audio Windows impossible")?;
        struct Com;
        impl Drop for Com { fn drop(&mut self) { unsafe { CoUninitialize(); } } }
        let _com = Com;
        let live = processes()?;
        let pids: HashSet<u32> = live.iter().filter(|p| target.pids.contains(&p.pid) && !is_own_process(&p.name)).map(|p| p.pid).collect();
        if pids.is_empty() { bail!("L'application cible s'est fermée : {}", target.label); }
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).context("Périphériques audio indisponibles")?;
        let outputs = devices.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE).context("Sorties audio indisponibles")?;
        let mut sessions = Vec::<ISimpleAudioVolume>::new();
        let mut first_error = None;
        for i in 0..outputs.GetCount()? {
            let result = (|| -> windows::core::Result<()> {
                let device = outputs.Item(i)?;
                let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
                let entries = manager.GetSessionEnumerator()?;
                for index in 0..entries.GetCount()? {
                    let control = match entries.GetSession(index) { Ok(v) => v, Err(_) => continue };
                    let extended: IAudioSessionControl2 = match control.cast() { Ok(v) => v, Err(_) => continue };
                    let pid = match extended.GetProcessId() { Ok(v) => v, Err(_) => continue };
                    if pid != 0 && pids.contains(&pid) {
                        if let Ok(volume) = control.cast::<ISimpleAudioVolume>() { sessions.push(volume); }
                    }
                }
                Ok(())
            })();
            if let Err(e) = result { first_error = Some(e); } // sortie débranchée pendant l'énumération
        }
        if sessions.is_empty() {
            if let Some(error) = first_error { return Err(anyhow!("Aucune session audio pour {} (sortie déconnectée : {error})", target.label)); }
            bail!("Aucune session audio pour {} : aucun son modifié", target.label);
        }
        // Read all mutes first to decide a coherent group toggle before writing.
        let mute_states = if action == "toggle_mute" {
            sessions.iter().map(|s| s.GetMute().map(|v| v.as_bool())).collect::<windows::core::Result<Vec<_>>>()
                .context("Impossible de lire l'état muet des sessions")?
        } else { vec![] };
        let mute = should_mute(&mute_states);
        let context = windows::core::GUID::from_u128(0xe11c6b19_89ea_441e_8911_1ee94a8ac463);
        let mut outcomes = Vec::new();
        for session in &sessions {
            match action {
                "mute" | "unmute" | "toggle_mute" => {
                    let value = if action == "toggle_mute" { mute } else { action == "mute" };
                    session.SetMute(value, &context).context("Session audio fermée ou sortie déconnectée")?;
                    outcomes.push(if value { "muette".to_owned() } else { "rétablie".to_owned() });
                }
                "up" | "down" | "set" => {
                    let current = session.GetMasterVolume().context("Session audio fermée")?;
                    let value = adjust(current, action, step, level);
                    session.SetMasterVolume(value, &context).context("Session audio fermée ou sortie déconnectée")?;
                    outcomes.push(format!("{} %", (value * 100.0).round() as u8));
                }
                _ => bail!("Action de volume inconnue"),
            }
        }
        Ok(format!("{} : {} ({} session(s))", target.label, outcomes.join(", "), sessions.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn point_changes_and_bounds() {
        assert!((adjust(0.40, "up", 5, 0) - 0.45).abs() < 0.0001);
        assert!((adjust(0.40, "down", 5, 0) - 0.35).abs() < 0.0001);
        assert_eq!(adjust(0.99, "up", 5, 0), 1.);
        assert_eq!(adjust(0.01, "down", 5, 0), 0.);
        assert_eq!(adjust(0.45, "set", 5, 0), 0.);
        assert_eq!(adjust(0.45, "set", 5, 100), 1.);
    }
    #[test] fn step_defaults_validation_and_summary() {
        use crate::model::{validate_key, Config, Key, Step};
        let step: Step = serde_json::from_str(r#"{"type":"app_volume"}"#).unwrap();
        assert!(matches!(&step, Step::AppVolume { target, action, step: 5, level: 0, include_children: true } if target == "foreground" && action == "up"));
        assert_eq!(step.app_volume_summary().unwrap(), "Augmenter le volume de l’application active de 5 points");
        let back: Step = serde_json::from_str(&serde_json::to_string(&step).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(&back).unwrap(), serde_json::to_value(&step).unwrap());
        let c = Config::default();
        let key = |s: Step| Key { steps: vec![s], ..Key::default() };
        assert!(validate_key(&key(step.clone()), &c).is_ok());
        for bad in [
            r#"{"type":"app_volume","target":"C:\\x\\chrome.exe"}"#, r#"{"type":"app_volume","target":"chrome"}"#,
            r#"{"type":"app_volume","action":"louder"}"#, r#"{"type":"app_volume","step":0}"#, r#"{"type":"app_volume","level":101}"#,
        ] { assert!(validate_key(&key(serde_json::from_str(bad).unwrap()), &c).is_err(), "{bad}"); }
        assert!(serde_json::from_str::<Step>(r#"{"type":"app_volume","master":true}"#).is_err());
    }
    #[test] fn toggle_multiple_sessions() {
        assert!(should_mute(&[true, false, true]));
        assert!(!should_mute(&[true, true]));
    }
}
