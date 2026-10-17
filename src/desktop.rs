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
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub profile: String,
    #[serde(default)]
    pub page: String,
    pub key: usize,
    pub keys: String,
}
pub enum Command {
    Hotkeys(Vec<(usize, String)>),
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
        let tip = wide("DeckForge · FIFINE D6");
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
        while !stop {
            while let Ok(command) = rx.try_recv() {
                match command {
                    Command::Window(window) => state.main.set(window),
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
    fn global_hotkeys_require_one_main_key_and_reject_reserved_keys() {
        assert!(chord("CTRL+ALT+F9").is_ok());
        for text in ["CTRL", "A", "WIN+E", "CTRL+F12", "CTRL+A+B", ""] {
            assert!(chord(text).is_err(), "{text}");
        }
    }
}
