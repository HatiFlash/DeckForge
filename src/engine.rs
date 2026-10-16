use crate::{
    model::{media_vk, parse_hotkey, Key, Step},
    Event,
};
use anyhow::{bail, Result};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{BOOL, HWND, LPARAM},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

pub struct Engine {
    pub running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    tx: Sender<Event>,
}
impl Engine {
    pub fn new(tx: Sender<Event>) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(AtomicBool::new(false)),
            tx,
        }
    }
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
    pub fn start(&self, key: Key, dry: bool, initial_delay: u64) {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            let _ = self.tx.send(Event::Log(
                "Macro déjà en cours : nouvel appui ignoré".into(),
            ));
            return;
        }
        self.cancel.store(false, Ordering::SeqCst);
        let running = self.running.clone();
        let cancel = self.cancel.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if !dry {
                    wait(initial_delay, &cancel)?;
                }
                run(&key, dry, &cancel, &tx)
            }));
            let message = match result {
                Ok(Ok(())) => "Macro terminée".into(),
                Ok(Err(e)) => format!("Macro arrêtée : {e}"),
                Err(_) => "Erreur interne : macro interrompue".into(),
            };
            let _ = tx.send(Event::Log(message));
            running.store(false, Ordering::SeqCst);
        });
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}

fn emergency() -> bool {
    unsafe {
        GetAsyncKeyState(0x11) < 0 && GetAsyncKeyState(0x12) < 0 && GetAsyncKeyState(0x7B) < 0
    }
}
fn check(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::SeqCst) || emergency() {
        bail!("annulation demandée");
    }
    Ok(())
}
fn wait(ms: u64, cancel: &AtomicBool) -> Result<()> {
    let end = Instant::now() + Duration::from_millis(ms);
    loop {
        check(cancel)?;
        let remaining = end.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(());
        }
        std::thread::sleep(remaining.min(Duration::from_millis(20)));
    }
}
fn run(key: &Key, dry: bool, cancel: &AtomicBool, tx: &Sender<Event>) -> Result<()> {
    let _ = tx.send(Event::Log(format!(
        "{} — {} ({} répétition(s))",
        if dry { "SIMULATION" } else { "EXÉCUTION" },
        key.label,
        key.repeat
    )));
    // Simulation previews one iteration even if the real macro repeats many times.
    for cycle in 0..if dry { 1 } else { key.repeat } {
        for (idx, step) in key.steps.iter().enumerate() {
            check(cancel)?;
            let _ = tx.send(Event::Log(format!(
                "Tour {} · étape {} : {}",
                cycle + 1,
                idx + 1,
                step.name()
            )));
            if dry {
                continue;
            }
            match step {
                Step::Delay { ms } => wait(*ms, cancel)?,
                Step::Hotkey { keys } => hotkey(&parse_hotkey(keys)?)?,
                Step::Text { text } => {
                    for unit in text.encode_utf16() {
                        check(cancel)?;
                        input(0, unit, KEYEVENTF_UNICODE)?;
                        input(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)?;
                    }
                }
                Step::Launch { program, args } => {
                    let mut child = std::process::Command::new(program).args(args).spawn()?;
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                }
                Step::Url { url } => {
                    let mut child = std::process::Command::new("explorer.exe")
                        .arg(url)
                        .spawn()?;
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                }
                Step::Media { command } => {
                    hotkey(&[media_vk(command)
                        .ok_or_else(|| anyhow::anyhow!("Commande média invalide"))?])?
                }
                Step::WaitWindow {
                    title,
                    timeout_ms,
                    focus,
                } => {
                    let deadline = Instant::now() + Duration::from_millis(*timeout_ms);
                    loop {
                        check(cancel)?;
                        if let Some(window) = find_window(title) {
                            if *focus {
                                unsafe {
                                    if IsIconic(window) != 0 {
                                        ShowWindow(window, SW_RESTORE);
                                    }
                                    if SetForegroundWindow(window) == 0 {
                                        bail!("Windows a refusé la mise au premier plan");
                                    }
                                }
                            }
                            break;
                        }
                        if Instant::now() >= deadline {
                            bail!("fenêtre introuvable après {timeout_ms} ms");
                        }
                        wait(50, cancel)?;
                    }
                }
                Step::Profile { name } => {
                    let _ = tx.send(Event::Profile(name.clone()));
                }
                Step::Page { .. }
                | Step::Folder { .. }
                | Step::Back
                | Step::PageNext
                | Step::PagePrevious => {
                    let _ = tx.send(Event::Navigate(step.clone()));
                    return Ok(());
                }
            }
            wait(10, cancel)?;
        }
    }
    Ok(())
}
fn input(vk: u16, scan: u16, flags: u32) -> Result<()> {
    let flags = flags
        | if [
            0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2D, 0x2E, 0x5B,
        ]
        .contains(&vk)
        {
            KEYEVENTF_EXTENDEDKEY
        } else {
            0
        };
    let i = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    if unsafe { SendInput(1, &i, std::mem::size_of::<INPUT>() as i32) } != 1 {
        bail!("Windows refuse la saisie (application élevée ou session verrouillée)");
    }
    Ok(())
}
struct Held(Vec<u16>);
impl Drop for Held {
    fn drop(&mut self) {
        for &vk in self.0.iter().rev() {
            let _ = input(vk, 0, KEYEVENTF_KEYUP);
        }
    }
}
fn hotkey(keys: &[u16]) -> Result<()> {
    let mut held = Held(Vec::new());
    for &vk in keys {
        input(vk, 0, 0)?;
        held.0.push(vk);
    }
    std::thread::sleep(Duration::from_millis(15));
    Ok(())
}
struct Search {
    query: String,
    found: Option<HWND>,
}
unsafe extern "system" fn enum_window(hwnd: HWND, data: LPARAM) -> BOOL {
    let search = &mut *(data as *mut Search);
    if IsWindowVisible(hwnd) == 0 {
        return 1;
    }
    let mut text = [0u16; 2048];
    let count = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
    if count > 0
        && String::from_utf16_lossy(&text[..count as usize])
            .to_lowercase()
            .contains(&search.query)
    {
        search.found = Some(hwnd);
        return 0;
    }
    1
}
fn find_window(query: &str) -> Option<HWND> {
    let mut search = Search {
        query: query.to_lowercase(),
        found: None,
    };
    unsafe {
        EnumWindows(Some(enum_window), &mut search as *mut _ as isize);
    }
    search.found
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_interrupts_long_delay() {
        let cancelled = AtomicBool::new(true);
        let t = Instant::now();
        assert!(wait(60_000, &cancelled).is_err());
        assert!(t.elapsed() < Duration::from_millis(100));
    }
    #[test]
    fn simulation_has_no_profile_side_effects() {
        let (tx, rx) = std::sync::mpsc::channel();
        let k = Key {
            label: "test".into(),
            repeat: 1000,
            steps: vec![
                Step::Profile {
                    name: "missing".into(),
                },
                Step::Delay { ms: 60_000 },
            ],
            ..Key::default()
        };
        run(&k, true, &AtomicBool::new(false), &tx).unwrap();
        assert_eq!(
            rx.try_iter()
                .filter(|e| matches!(e, Event::Profile(_)))
                .count(),
            0
        );
    }
    #[test]
    fn repeats_preserve_action_order() {
        let (tx, rx) = std::sync::mpsc::channel();
        let k = Key {
            repeat: 3,
            steps: vec![
                Step::Profile { name: "A".into() },
                Step::Profile { name: "B".into() },
            ],
            ..Key::default()
        };
        run(&k, false, &AtomicBool::new(false), &tx).unwrap();
        let names = rx
            .try_iter()
            .filter_map(|e| {
                if let Event::Profile(n) = e {
                    Some(n)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(names, ["A", "B", "A", "B", "A", "B"]);
    }
    #[test]
    fn cancellation_during_delay_is_responsive() {
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            c.store(true, Ordering::SeqCst);
        });
        let start = Instant::now();
        assert!(wait(60_000, &cancel).is_err());
        thread.join().unwrap();
        assert!(start.elapsed() < Duration::from_secs(1));
    }
}
