//! Tray, startup registration and global hotkeys. No administrator rights required.
use crate::{model::parse_hotkey, Event};
use anyhow::{bail, Result};
use std::{
    sync::mpsc::{self, Sender},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{LibraryLoader::GetModuleHandleW, Registry::*},
    UI::{Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
};
const CLASS: &str = "DeckForgeTray04";
const CALLBACK: u32 = WM_APP + 41;
const SHOW: u32 = WM_APP + 42;
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub profile: String,
    #[serde(default)] pub page: String,
    pub key: usize,
    #[serde(default)] pub keys: String,
    #[serde(default = "hotkey_kind")] pub kind: String,
    #[serde(default)] pub value: String,
    #[serde(default)] pub days: Vec<u8>, // 0 = dimanche, 6 = samedi ; vide = chaque jour
    #[serde(default)] pub minutes: u32,
    #[serde(default)] pub input: bool,
}
fn hotkey_kind() -> String { "hotkey".into() }
impl Trigger {
    pub fn hotkey(profile: String, page: String, key: usize, keys: String) -> Self {
        Self { profile, page, key, keys, kind: hotkey_kind(), value: String::new(), days: vec![], minutes: 0, input: false }
    }
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(self.key < 15 && !self.profile.trim().is_empty() && self.profile.len() <= 128 && self.page.len() <= 128, "Destination du déclencheur invalide");
        match self.kind.as_str() {
            "hotkey" => { chord(&self.keys)?; }
            "app_open" | "app_close" => anyhow::ensure!(self.value.len() <= 128 && crate::conditions::valid_exe(&self.value), "Exécutable du déclencheur invalide"),
            "audio_connect" | "audio_disconnect" => anyhow::ensure!(!self.value.trim().is_empty() && self.value.len() <= 256 && !self.value.chars().any(char::is_control), "Nom de périphérique invalide"),
            "clock" => {
                let bytes = self.value.as_bytes();
                anyhow::ensure!(bytes.len() == 5 && bytes[2] == b':' && [0, 1, 3, 4].iter().all(|&i| bytes[i].is_ascii_digit()) && self.value[0..2].parse::<u8>().unwrap() < 24 && self.value[3..5].parse::<u8>().unwrap() < 60 && self.days.len() <= 7 && self.days.iter().all(|d| *d <= 6) && { let mut d = self.days.clone(); d.sort(); d.dedup(); d.len() == self.days.len() }, "Heure ou jours invalides (HH:MM, dimanche = 0)");
            }
            "timer" => anyhow::ensure!((1..=1440).contains(&self.minutes), "Minuteur : 1 à 1440 minutes"),
            _ => anyhow::bail!("Type de déclencheur inconnu"),
        }
        Ok(())
    }
    pub fn description(&self) -> String {
        match self.kind.as_str() {
            "app_open" => format!("ouverture de {}", self.value), "app_close" => format!("fermeture de {}", self.value),
            "audio_connect" => format!("branchement audio {}", self.value), "audio_disconnect" => format!("débranchement audio {}", self.value),
            "clock" => format!("{} (jours : {})", self.value, if self.days.is_empty() { "tous".into() } else { self.days.iter().map(u8::to_string).collect::<Vec<_>>().join(",") }),
            "timer" => format!("toutes les {} minutes", self.minutes), _ => format!("raccourci {}", self.keys),
        }
    }
}
/// Transition pure : aucune émission lors du premier échantillon (baseline).
pub fn transition(kind: &str, before: Option<bool>, now: bool) -> bool {
    match kind { "app_open" | "audio_connect" => before == Some(false) && now,
        "app_close" | "audio_disconnect" => before == Some(true) && !now, _ => false }
}
pub fn clock_transition(before: Option<(u16,u16,u16,u16,u16)>, now: (u16,u16,u16,u16,u16), weekday: u8, trigger: &Trigger) -> bool {
    before.is_some_and(|old| old != now) && (trigger.days.is_empty() || trigger.days.contains(&weekday))
        && trigger.value == format!("{:02}:{:02}", now.3, now.4)
}
pub fn timer_due(elapsed: Duration, minutes: u32) -> bool { minutes > 0 && elapsed >= Duration::from_secs(u64::from(minutes) * 60) }
pub enum Command {
    Hotkeys(Vec<(usize, String)>),
    Triggers(Vec<Trigger>),
    Window(isize),
    Notify(String, String),
    Stop,
}
pub struct Desktop {
    pub tx: Sender<Command>,
}
impl Drop for Desktop {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Stop);
    }
}
pub fn chord(text: &str) -> Result<(u32, u32)> {
    let codes = parse_hotkey(text)?;
    let mut modifiers = MOD_NOREPEAT;
    let mut key = None;
    for vk in codes {
        match vk {
            0x11 => modifiers |= MOD_CONTROL,
            0x10 => modifiers |= MOD_SHIFT,
            0x12 => modifiers |= MOD_ALT,
            0x5b => bail!("Les raccourcis Win sont réservés à Windows"),
            _ => {
                if key.replace(vk as u32).is_some() {
                    bail!("Un raccourci global contient une seule touche principale");
                }
            }
        }
    }
    let key = key.ok_or_else(|| anyhow::anyhow!("Ajoute une touche après Ctrl, Alt ou Maj"))?;
    anyhow::ensure!(
        modifiers != MOD_NOREPEAT && key != 0x7b,
        "Utilise Ctrl, Alt ou Maj avec une touche autre que F12"
    );
    Ok((modifiers, key))
}
struct State {
    tx: Sender<Event>,
    ctx: eframe::egui::Context,
    icon: NOTIFYICONDATAW,
    taskbar: u32,
    main: std::cell::Cell<isize>,
}
impl State {
    fn send(&self, event: Event) {
        // Restore a hidden/minimized native window only for explicit Open/Quit; never on ordinary events.
        if matches!(event, Event::Open | Event::Quit) {
            unsafe {
                let window = self.main.get() as HWND;
                if !window.is_null() && (matches!(event, Event::Quit) || IsWindowVisible(window) == 0 || IsIconic(window) != 0) { ShowWindowAsync(window, SW_RESTORE); }
            }
        }
        let _ = self.tx.send(event);
        self.ctx.request_repaint();
    }
}
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
    if !p.is_null() {
        let s = &*p;
        if msg == s.taskbar {
            Shell_NotifyIconW(NIM_ADD, &s.icon);
            return 0;
        }
        if msg == SHOW {
            s.send(Event::Open);
            return 0;
        }
        if msg == CALLBACK {
            match l as u32 {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => s.send(Event::Open),
                WM_RBUTTONUP => {
                    let menu = CreatePopupMenu();
                    for (id, label) in [
                        (1, "Ouvrir DeckForge"),
                        (2, "Activer / suspendre les macros"),
                        (3, "Quitter DeckForge"),
                    ] {
                        AppendMenuW(menu, MF_STRING, id, wide(label).as_ptr());
                    }
                    let mut point: POINT = std::mem::zeroed();
                    GetCursorPos(&mut point);
                    SetForegroundWindow(hwnd);
                    let selected = TrackPopupMenu(
                        menu,
                        TPM_RETURNCMD | TPM_RIGHTBUTTON,
                        point.x,
                        point.y,
                        0,
                        hwnd,
                        std::ptr::null(),
                    );
                    DestroyMenu(menu);
                    match selected {
                        1 => s.send(Event::Open),
                        2 => s.send(Event::Toggle),
                        3 => s.send(Event::Quit),
                        _ => {}
                    }
                    PostMessageW(hwnd, WM_NULL, 0, 0);
                }
                _ => {}
            }
            return 0;
        }
    }
    DefWindowProcW(hwnd, msg, w, l)
}
pub fn show_existing() -> bool {
    unsafe {
        let h = FindWindowW(wide(CLASS).as_ptr(), std::ptr::null());
        !h.is_null() && PostMessageW(h, SHOW, 0, 0) != 0
    }
}
/// None on inaccessible windows; DeckForge itself is ignored instead of losing the base profile.
pub fn foreground_exe() -> Option<String> {
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() { return None; }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 || pid == std::process::id() { return None; }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() { return None; }
        let mut buffer = [0u16; 32768];
        let mut len = buffer.len() as u32;
        let result = if QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut len) == 0 { None } else {
            std::path::Path::new(&String::from_utf16_lossy(&buffer[..len as usize]))
                .file_name().map(|s| s.to_string_lossy().to_string())
        };
        CloseHandle(process);
        result
    }
}
pub fn spawn(tx: Sender<Event>, ctx: eframe::egui::Context) -> Desktop {
    let (cmd, rx) = mpsc::channel();
    std::thread::spawn(move || unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide(CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        // A hidden top-level window receives TaskbarCreated after Explorer restarts.
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            let _ = tx.send(Event::Log("Icône de notification indisponible".into()));
            let _ = tx.send(Event::TrayReady(false));
            ctx.request_repaint();
            return;
        }
        let mut icon: NOTIFYICONDATAW = std::mem::zeroed();
        icon.cbSize = std::mem::size_of_val(&icon) as u32;
        icon.hWnd = hwnd;
        icon.uID = 1;
        icon.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        icon.uCallbackMessage = CALLBACK;
        let mut pixels = crate::theme::app_icon().rgba;
        let mut mask = vec![0u8; 128];
        for (i, pixel) in pixels.chunks_exact_mut(4).enumerate() {
            pixel.swap(0, 2);
            if pixel[3] == 0 {
                mask[i / 8] |= 0x80 >> (i % 8);
            }
        }
        icon.hIcon = CreateIcon(instance, 32, 32, 1, 32, mask.as_ptr(), pixels.as_ptr());
        let own_icon = !icon.hIcon.is_null();
        if !own_icon {
            icon.hIcon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
        }
        let tip = wide("DeckForge");
        icon.szTip[..tip.len()].copy_from_slice(&tip);
        let state = Box::new(State {
            tx: tx.clone(),
            ctx,
            icon,
            taskbar: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
            main: std::cell::Cell::new(0),
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&*state as *const State) as isize);
        let ready = Shell_NotifyIconW(NIM_ADD, &state.icon) != 0;
        state.send(Event::TrayReady(ready));
        if !ready {
            state.send(Event::Log(
                "Windows n’a pas créé l’icône de notification".into(),
            ));
        }
        let mut registered = Vec::new();
        let mut last_foreground = None;
        let mut last_scan = Instant::now() - Duration::from_secs(1);
        let mut stop = false;
        let mut triggers: Vec<Trigger> = Vec::new();
        let mut baselines: Vec<Option<bool>> = Vec::new();
        let mut timers: Vec<Instant> = Vec::new();
        let mut last_clock: Option<(u16,u16,u16,u16,u16)> = None;
        let mut last_trigger_scan = Instant::now();
        while !stop {
            while let Ok(command) = rx.try_recv() {
                match command {
                    Command::Window(window) => state.main.set(window),
                    Command::Triggers(next) => {
                        triggers = next.into_iter().filter(|t| t.kind != "hotkey" && t.validate().is_ok()).collect();
                        baselines = vec![None; triggers.len()];
                        timers = vec![Instant::now(); triggers.len()];
                        last_clock = None;
                    },
                    Command::Notify(title, message) => {
                        let mut balloon = state.icon;
                        balloon.uFlags = NIF_INFO;
                        for (target, text) in [(&mut balloon.szInfoTitle[..], title.as_str()), (&mut balloon.szInfo[..], message.as_str())] {
                            let utf16: Vec<u16> = text.encode_utf16().take(target.len().saturating_sub(1)).collect();
                            target[..utf16.len()].copy_from_slice(&utf16);
                            target[utf16.len()] = 0;
                        }
                        balloon.dwInfoFlags = NIIF_INFO | NIIF_NOSOUND;
                        if Shell_NotifyIconW(NIM_MODIFY, &balloon) == 0 {
                            state.send(Event::Log("Notification Windows indisponible ; message conservé dans le journal".into()));
                        }
                    },
                    Command::Stop => stop = true,
                    Command::Hotkeys(bindings) => {
                        for id in registered.drain(..) {
                            UnregisterHotKey(hwnd, id);
                        }
                        for (index, text) in bindings {
                            match chord(&text) {
                                Ok((modifiers, key)) => {
                                    let id = index as i32 + 1;
                                    if RegisterHotKey(hwnd, id, modifiers, key) != 0 {
                                        registered.push(id);
                                    } else {
                                        state.send(Event::Log(format!("Raccourci {text} indisponible : déjà utilisé ou réservé par Windows")));
                                    }
                                }
                                Err(e) => state.send(Event::Log(format!("Raccourci {text} : {e}"))),
                            }
                        }
                    }
                }
            }
            if last_scan.elapsed() >= Duration::from_millis(500) {
                last_scan = Instant::now();
                if let Some(exe) = foreground_exe() {
                    if last_foreground.as_ref() != Some(&exe) {
                        last_foreground = Some(exe.clone());
                        state.send(Event::Foreground(Some(exe)));
                    }
                }
            }
            if !triggers.is_empty() && last_trigger_scan.elapsed() >= Duration::from_secs(1) {
                last_trigger_scan = Instant::now();
                let processes = if triggers.iter().any(|t| t.kind.starts_with("app_")) {
                    crate::conditions::running_processes().ok()
                } else { None };
                let audio = if triggers.iter().any(|t| t.kind.starts_with("audio_")) {
                    crate::audio::snapshot().ok()
                } else { None };
                let mut local: windows_sys::Win32::Foundation::SYSTEMTIME = std::mem::zeroed();
                windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut local);
                let clock = (local.wYear, local.wMonth, local.wDay, local.wHour, local.wMinute);
                for (i, trigger) in triggers.iter().enumerate() {
                    let present = match trigger.kind.as_str() {
                        "app_open" | "app_close" => processes.as_ref().map(|list| list.iter().any(|(_, exe)| exe.eq_ignore_ascii_case(&trigger.value))),
                        "audio_connect" | "audio_disconnect" => audio.as_ref().map(|s| s.devices.iter().any(|d| d.input == trigger.input && d.name.to_lowercase().contains(&trigger.value.to_lowercase()))),
                        _ => None,
                    };
                    let fired = if let Some(present) = present {
                        let fire = transition(&trigger.kind, baselines[i], present);
                        baselines[i] = Some(present);
                        fire
                    } else if trigger.kind == "clock" {
                        clock_transition(last_clock, clock, local.wDayOfWeek as u8, trigger)
                    } else if trigger.kind == "timer" && timer_due(timers[i].elapsed(), trigger.minutes) {
                        timers[i] = Instant::now(); true
                    } else { false };
                    if fired { state.send(Event::Automatic(trigger.clone(), Instant::now())); }
                }
                last_clock = Some(clock);
            }
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, hwnd, 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_HOTKEY {
                    state.send(Event::Hotkey(msg.wParam.saturating_sub(1), Instant::now()));
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        for id in registered {
            UnregisterHotKey(hwnd, id);
        }
        Shell_NotifyIconW(NIM_DELETE, &state.icon);
        if own_icon {
            DestroyIcon(state.icon.hIcon);
        }
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        DestroyWindow(hwnd);
        UnregisterClassW(class.as_ptr(), instance);
    });
    Desktop { tx: cmd }
}
pub fn startup(enable: bool) -> Result<()> {
    unsafe {
        let mut key = std::ptr::null_mut();
        let code = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run").as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        anyhow::ensure!(code == ERROR_SUCCESS, "Démarrage Windows : erreur {code}");
        let name = wide("DeckForge");
        let code = if enable {
            let exe = std::env::current_exe()?;
            let command = wide(&format!("\"{}\" --background", exe.display()));
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                command.as_ptr().cast(),
                (command.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(key, name.as_ptr())
        };
        RegCloseKey(key);
        anyhow::ensure!(
            code == ERROR_SUCCESS || (!enable && code == ERROR_FILE_NOT_FOUND),
            "Démarrage Windows : erreur {code}"
        );
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_transitions_and_legacy_preferences() {
        let old = r#"{"profile":"Défaut","page":"","key":2,"keys":"CTRL+ALT+F9"}"#;
        let legacy: Trigger = serde_json::from_str(old).unwrap();
        assert_eq!(legacy.kind, "hotkey");
        assert!(legacy.validate().is_ok());
        assert_eq!(serde_json::from_str::<Trigger>(&serde_json::to_string(&legacy).unwrap()).unwrap(), legacy);
        assert!(serde_json::from_str::<Trigger>(r#"{"profile":"P","key":0,"keys":"CTRL+A","extra":true}"#).is_err());
        assert!(!transition("app_open", None, true));
        assert!(transition("app_open", Some(false), true));
        assert!(!transition("app_open", Some(true), true));
        assert!(transition("app_close", Some(true), false));
        assert!(transition("audio_connect", Some(false), true));
        assert!(transition("audio_disconnect", Some(true), false));
        let mut clock = Trigger::hotkey("P".into(), String::new(), 2, String::new());
        clock.kind = "clock".into(); clock.value = "13:05".into(); clock.days = vec![1, 3];
        assert!(clock.validate().is_ok());
        let minute = (2026, 10, 19, 13, 5);
        assert!(!clock_transition(None, minute, 1, &clock));
        assert!(!clock_transition(Some(minute), minute, 1, &clock));
        assert!(!clock_transition(Some((2026, 10, 19, 13, 4)), minute, 2, &clock));
        assert!(clock_transition(Some((2026, 10, 19, 13, 4)), minute, 1, &clock));
        clock.value = "25:00".into(); assert!(clock.validate().is_err());
        clock.value = "13:05".into(); clock.days = vec![7]; assert!(clock.validate().is_err());
        let mut timer = Trigger::hotkey("P".into(), String::new(), 2, String::new());
        timer.kind = "timer".into(); timer.minutes = 5;
        assert!(timer.validate().is_ok());
        assert!(!timer_due(Duration::from_secs(299), timer.minutes));
        assert!(timer_due(Duration::from_secs(300), timer.minutes));
        timer.minutes = 0; assert!(timer.validate().is_err());
        timer.kind = "app_open".into(); timer.value = "bad/path.exe".into(); assert!(timer.validate().is_err());
    }
    #[test]
    fn global_hotkeys_require_one_main_key_and_reject_reserved_keys() {
        assert!(chord("CTRL+ALT+F9").is_ok());
        for text in ["CTRL", "A", "WIN+E", "CTRL+F12", "CTRL+A+B", ""] {
            assert!(chord(text).is_err(), "{text}");
        }
    }
}
