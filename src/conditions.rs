//! Conditions en lecture seule : calculs séparés des observations Windows.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use windows::{core::Interface, Win32::{Media::Audio::*, System::Com::*}};
use windows_sys::Win32::{Foundation::CloseHandle, System::Diagnostics::ToolHelp::*, UI::WindowsAndMessaging::*};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    ForegroundApp { #[serde(default = "default_exe")] exe: String },
    AppRunning { #[serde(default = "default_exe")] exe: String },
    WindowOpen { #[serde(default = "default_title")] title: String },
    AppAudioPlaying { #[serde(default = "default_target")] exe: String },
    AppMuted { #[serde(default = "default_target")] exe: String },
    AppVolumeCompare { #[serde(default = "default_target")] exe: String, #[serde(default = "default_above")] op: String, #[serde(default = "default_level")] value: u8 },
    DeviceMuted { #[serde(default)] input: bool, #[serde(default = "default_device")] device: String },
    DeviceVolumeCompare { #[serde(default)] input: bool, #[serde(default = "default_device")] device: String, #[serde(default = "default_above")] op: String, #[serde(default = "default_level")] value: u8 },
    AudioDevicePresent { #[serde(default = "default_title")] name_contains: String, #[serde(default)] input: bool },
    DefaultOutputIs { #[serde(default = "default_title")] name_contains: String },
    VariableCompare { #[serde(default = "default_name")] name: String, #[serde(default = "default_equals")] op: String, #[serde(default)] value: String },
    Not { condition: Box<Condition> },
    All { #[serde(default)] conditions: Vec<Condition> },
    Any { #[serde(default)] conditions: Vec<Condition> },
}
fn default_exe() -> String { "explorer.exe".into() }
fn default_title() -> String { "Casque".into() }
fn default_name() -> String { "mode".into() }
fn default_target() -> String { "foreground".into() }
fn default_device() -> String { "default".into() }
fn default_above() -> String { "above".into() }
fn default_equals() -> String { "equals".into() }
fn default_level() -> u8 { 50 }
impl Default for Condition { fn default() -> Self { Self::ForegroundApp { exe: default_exe() } } }

pub fn compare_number(actual: f32, op: &str, value: f32) -> bool {
    match op { "above" => actual > value, "below" => actual < value, _ => false }
}
pub fn compare_variable(actual: &str, op: &str, value: &str) -> bool {
    match op {
        "equals" => actual == value, "not_equals" => actual != value,
        "contains" => actual.contains(value),
        "greater" | "less" => {
            let order = match (actual.parse::<f64>(), value.parse::<f64>()) {
                (Ok(a), Ok(b)) if a.is_finite() && b.is_finite() => a.partial_cmp(&b),
                _ => Some(actual.cmp(value)),
            };
            matches!((op, order), ("greater", Some(std::cmp::Ordering::Greater)) | ("less", Some(std::cmp::Ordering::Less)))
        },
        _ => false,
    }
}
pub fn decide_not(value: bool) -> bool { !value }
pub fn decide_all(values: &[bool]) -> bool { values.iter().all(|&v| v) }
pub fn decide_any(values: &[bool]) -> bool { values.iter().any(|&v| v) }
fn valid_exe(exe: &str) -> bool {
    exe.len() <= 260 && exe.trim() == exe && exe.to_ascii_lowercase().ends_with(".exe")
        && !exe.contains(['/', '\\']) && !exe.chars().any(char::is_control)
}
fn valid_text(s: &str, max: usize) -> bool { !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control) }
impl Condition {
    pub fn kind(&self) -> &'static str {
        match self { Self::ForegroundApp { .. } => "foreground_app", Self::AppRunning { .. } => "app_running", Self::WindowOpen { .. } => "window_open", Self::AppAudioPlaying { .. } => "app_audio_playing", Self::AppMuted { .. } => "app_muted", Self::AppVolumeCompare { .. } => "app_volume_compare", Self::DeviceMuted { .. } => "device_muted", Self::DeviceVolumeCompare { .. } => "device_volume_compare", Self::AudioDevicePresent { .. } => "audio_device_present", Self::DefaultOutputIs { .. } => "default_output_is", Self::VariableCompare { .. } => "variable_compare", Self::Not { .. } => "not", Self::All { .. } => "all", Self::Any { .. } => "any" }
    }
    pub fn label(&self) -> &'static str {
        match self { Self::ForegroundApp { .. } => "Application au premier plan", Self::AppRunning { .. } => "Application ouverte (même en arrière-plan)", Self::WindowOpen { .. } => "Fenêtre ouverte", Self::AppAudioPlaying { .. } => "Session audio active", Self::AppMuted { .. } => "Application muette", Self::AppVolumeCompare { .. } => "Volume d'application", Self::DeviceMuted { .. } => "Périphérique muet", Self::DeviceVolumeCompare { .. } => "Volume du périphérique", Self::AudioDevicePresent { .. } => "Périphérique audio actif", Self::DefaultOutputIs { .. } => "Sortie audio par défaut", Self::VariableCompare { .. } => "Comparer une variable", Self::Not { .. } => "NON", Self::All { .. } => "Toutes (ET)", Self::Any { .. } => "Au moins une (OU)" }
    }
    pub fn summary(&self) -> String {
        match self { Self::ForegroundApp { exe } => format!("{exe} au premier plan"), Self::AppRunning { exe } => format!("{exe} ouvert (arrière-plan inclus)"), Self::WindowOpen { title } => format!("fenêtre contenant « {title} » ouverte"), Self::AppAudioPlaying { exe } => format!("session audio de {exe} active"), Self::AppMuted { exe } => format!("{exe} muet"), Self::AppVolumeCompare { exe, op, value } => format!("volume de {exe} {} {value} %", if op == "above" { ">" } else { "<" }), Self::DeviceMuted { input, .. } => format!("{} muet", if *input { "microphone" } else { "sortie audio" }), Self::DeviceVolumeCompare { input, op, value, .. } => format!("volume {} {} {value} %", if *input { "microphone" } else { "sortie audio" }, if op == "above" { ">" } else { "<" }), Self::AudioDevicePresent { name_contains, input } => format!("{} actif contenant « {name_contains} »", if *input { "microphone" } else { "périphérique de sortie" }), Self::DefaultOutputIs { name_contains } => format!("sortie par défaut contenant « {name_contains} »"), Self::VariableCompare { name, op, value } => format!("variable {name} {op} « {value} »"), Self::Not { condition } => format!("NON ({})", condition.summary()), Self::All { conditions } => format!("toutes les {} conditions", conditions.len()), Self::Any { conditions } => format!("une des {} conditions", conditions.len()) }
    }
    pub fn validate(&self) -> Result<()> { self.validate_at(0) }
    fn validate_at(&self, depth: usize) -> Result<()> {
        anyhow::ensure!(depth <= 8, "8 niveaux de conditions maximum");
        match self {
            Self::ForegroundApp { exe } | Self::AppRunning { exe } => anyhow::ensure!(valid_exe(exe), "Nom d'exécutable invalide"),
            Self::AppAudioPlaying { exe } | Self::AppMuted { exe } => anyhow::ensure!(exe == "foreground" || valid_exe(exe), "Cible audio invalide"),
            Self::AppVolumeCompare { exe, op, value } => anyhow::ensure!((exe == "foreground" || valid_exe(exe)) && ["above", "below"].contains(&op.as_str()) && *value <= 100, "Comparaison de volume d'application invalide"),
            Self::DeviceMuted { device, .. } => anyhow::ensure!(valid_text(device, 512), "Périphérique invalide"),
            Self::DeviceVolumeCompare { device, op, value, .. } => anyhow::ensure!(valid_text(device, 512) && ["above", "below"].contains(&op.as_str()) && *value <= 100, "Comparaison de volume du périphérique invalide"),
            Self::WindowOpen { title } => anyhow::ensure!(valid_text(title, 256), "Titre de fenêtre invalide"),
            Self::AudioDevicePresent { name_contains, .. } | Self::DefaultOutputIs { name_contains } => anyhow::ensure!(valid_text(name_contains, 256), "Nom de périphérique invalide"),
            Self::VariableCompare { name, op, value } => anyhow::ensure!(!name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && value.len() <= 4096 && ["equals", "not_equals", "contains", "greater", "less"].contains(&op.as_str()), "Comparaison de variable invalide"),
            Self::Not { condition } => condition.validate_at(depth + 1)?,
            Self::All { conditions } | Self::Any { conditions } => {
                anyhow::ensure!(!conditions.is_empty() && conditions.len() <= 32, "Entre 1 et 32 conditions requises");
                for condition in conditions { condition.validate_at(depth + 1)?; }
            },
        }
        Ok(())
    }
    pub fn evaluate(&self, vars: &BTreeMap<String, String>) -> Result<bool> {
        match self {
            Self::ForegroundApp { exe } => Ok(foreground_exe()?.is_some_and(|name| name.eq_ignore_ascii_case(exe))),
            Self::AppRunning { exe } => Ok(running_processes()?.iter().any(|(_, name)| name.eq_ignore_ascii_case(exe))),
            Self::WindowOpen { title } => Ok(crate::engine::find_window(title).is_some()),
            Self::VariableCompare { name, op, value } => Ok(compare_variable(vars.get(name).map(String::as_str).unwrap_or(""), op, value)),
            Self::Not { condition } => Ok(decide_not(condition.evaluate(vars)?)),
            Self::All { conditions } => { let mut values = Vec::new(); for c in conditions { values.push(c.evaluate(vars)?); if !decide_all(&values) { return Ok(false); } } Ok(decide_all(&values)) },
            Self::Any { conditions } => { let mut values = Vec::new(); for c in conditions { values.push(c.evaluate(vars)?); if decide_any(&values) { return Ok(true); } } Ok(decide_any(&values)) },
            Self::AudioDevicePresent { name_contains, input } => Ok(crate::audio::snapshot()?.devices.iter().any(|d| d.input == *input && contains_ci(&d.name, name_contains))),
            Self::DefaultOutputIs { name_contains } => { let s = crate::audio::snapshot()?; Ok(s.devices.iter().any(|d| d.id == s.output && contains_ci(&d.name, name_contains))) },
            Self::AppAudioPlaying { exe } => Ok(audio_sessions(exe)?.iter().any(|(active, _, _)| *active)),
            Self::AppMuted { exe } => Ok(audio_sessions(exe)?.iter().any(|(_, mute, _)| *mute)),
            Self::AppVolumeCompare { exe, op, value } => Ok(audio_sessions(exe)?.iter().any(|(_, _, volume)| compare_number(*volume * 100.0, op, f32::from(*value)))),
            Self::DeviceMuted { input, device } => Ok(device_state(*input, device)?.1),
            Self::DeviceVolumeCompare { input, device, op, value } => Ok(compare_number(device_state(*input, device)?.0 * 100.0, op, f32::from(*value))),
        }
    }
}
fn contains_ci(haystack: &str, needle: &str) -> bool { haystack.to_lowercase().contains(&needle.to_lowercase()) }
pub fn running_exes() -> Vec<String> {
    let mut names: Vec<_> = running_processes().unwrap_or_default().into_iter().map(|(_, n)| n).collect();
    names.sort_by_key(|s| s.to_lowercase()); names.dedup_by(|a, b| a.eq_ignore_ascii_case(b)); names
}
fn running_processes() -> Result<Vec<(u32, String)>> {
    unsafe {
        let handle = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if handle.is_null() || handle as isize == -1 { bail!("Lecture des processus Windows impossible"); }
        struct Guard(windows_sys::Win32::Foundation::HANDLE);
        impl Drop for Guard { fn drop(&mut self) { unsafe { CloseHandle(self.0); } } }
        let _guard = Guard(handle);
        let mut entry: PROCESSENTRY32W = std::mem::zeroed(); entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(handle, &mut entry) == 0 { bail!("Liste de processus indisponible"); }
        let mut result = Vec::new();
        loop {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            result.push((entry.th32ProcessID, String::from_utf16_lossy(&entry.szExeFile[..len])));
            if Process32NextW(handle, &mut entry) == 0 { break; }
        }
        Ok(result)
    }
}
fn foreground_exe() -> Result<Option<String>> {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid); }
    if pid == 0 { return Ok(None); }
    Ok(running_processes()?.into_iter().find(|(id, _)| *id == pid).map(|(_, name)| name))
}
fn target_pids(exe: &str) -> Result<HashSet<u32>> {
    if exe == "foreground" {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid); }
        return Ok(if pid == 0 || pid == std::process::id() { HashSet::new() } else { HashSet::from([pid]) });
    }
    Ok(running_processes()?.into_iter().filter(|(id, name)| *id != std::process::id() && name.eq_ignore_ascii_case(exe)).map(|(id, _)| id).collect())
}
fn audio_sessions(exe: &str) -> Result<Vec<(bool, bool, f32)>> {
    let pids = target_pids(exe)?;
    if pids.is_empty() { return Ok(Vec::new()); }
    let _com = Com::new()?;
    unsafe {
        let e: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let outputs = e.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        let mut states = Vec::new();
        for i in 0..outputs.GetCount()? {
            let manager: IAudioSessionManager2 = outputs.Item(i)?.Activate(CLSCTX_ALL, None)?;
            let list = manager.GetSessionEnumerator()?;
            for j in 0..list.GetCount()? {
                let session = list.GetSession(j)?;
                let control: IAudioSessionControl2 = session.cast()?;
                if !pids.contains(&control.GetProcessId()?) { continue; }
                let volume: ISimpleAudioVolume = session.cast()?;
                states.push((session.GetState()? == AudioSessionStateActive, volume.GetMute()?.as_bool(), volume.GetMasterVolume()?));
            }
        }
        Ok(states)
    }
}
struct Com;
impl Com { fn new() -> Result<Self> { unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok().context("Audio Windows indisponible")?; } Ok(Self) } }
impl Drop for Com { fn drop(&mut self) { unsafe { CoUninitialize(); } } }
fn device_state(input: bool, device: &str) -> Result<(f32, bool)> {
    let _com = Com::new()?;
    unsafe {
        let e: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let flow = if input { eCapture } else { eRender };
        let d = if device == "default" { e.GetDefaultAudioEndpoint(flow, eConsole)? } else {
            let endpoints = e.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)?;
            let mut found = None;
            for i in 0..endpoints.GetCount()? {
                let candidate = endpoints.Item(i)?;
                let id = candidate.GetId()?;
                let is_match = id.to_string()? == device;
                CoTaskMemFree(Some(id.0.cast()));
                if is_match { found = Some(candidate); break; }
            }
            found.context("Périphérique audio déconnecté ou indisponible")?
        };
        let v: windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume = d.Activate(CLSCTX_ALL, None)?;
        Ok((v.GetMasterVolumeLevelScalar()?, v.GetMute()?.as_bool()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn pure_comparisons_and_boolean_logic() {
        assert!(compare_number(51.0, "above", 50.0)); assert!(!compare_number(50.0, "above", 50.0));
        assert!(compare_variable("10", "greater", "2")); assert!(compare_variable("abc", "contains", "b"));
        assert!(decide_not(false) && decide_all(&[true, true]) && decide_any(&[false, true]));
    }
    #[test] fn defaults_roundtrip_and_invalid() {
        let c: Condition = serde_json::from_str(r#"{"kind":"app_volume_compare"}"#).unwrap();
        assert_eq!(c, serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap());
        assert!(c.validate().is_ok());
        assert!(serde_json::from_str::<Condition>(r#"{"kind":"app_running","unexpected":1}"#).is_err());
        assert!(Condition::All { conditions: vec![] }.validate().is_err());
        assert!(Condition::AppVolumeCompare { exe: "bad".into(), op: "above".into(), value: 100 }.validate().is_err());
    }
}
