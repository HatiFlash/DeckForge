use crate::{
    model::{media_vk, parse_hotkey, Key, Step},
    Event,
};
use anyhow::{bail, Context, Result};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, MONITOR_DEFAULTTONEAREST, MONITORINFO};
use windows_sys::Win32::{
    Foundation::{BOOL, HWND, LPARAM, RECT},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

thread_local! { static MOUSE_WINDOW: std::cell::Cell<Option<(i32, i32, i32, i32)>> = const { std::cell::Cell::new(None) }; }
thread_local! { static WORKSHOP_RUN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
thread_local! { static ACTIVE_KEY: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) }; }
static TOGGLE_STATES: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, bool>>> = std::sync::OnceLock::new();
fn toggle_states() -> &'static std::sync::Mutex<std::collections::HashMap<String, bool>> {
    TOGGLE_STATES.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}
pub fn identity(profile: &str, page: &str, index: usize) -> String { format!("{profile}\0{page}\0{index}") }
pub fn toggle_badge(id: &str, key: &Key) -> Option<String> {
    let index = key.steps.iter().position(|s| matches!(s.active(), Step::Toggle { .. }))?;
    let path = format!("{id}\0{index}");
    let states = toggle_states().lock().unwrap_or_else(|e| e.into_inner());
    Some(if *states.get(&path).unwrap_or(&false) { "2" } else { "1" }.to_owned() + "/2")
}
fn next_branch(path: &[usize]) -> usize {
    let id = ACTIVE_KEY.with(|id| id.borrow().clone());
    let id = format!("{id}\0{}", path.iter().map(usize::to_string).collect::<Vec<_>>().join("."));
    let mut states = toggle_states().lock().unwrap_or_else(|e| e.into_inner());
    if states.len() > 2048 { states.clear(); }
    let next = states.entry(id).or_insert(false);
    let branch = usize::from(*next);
    *next = !*next;
    branch
}
// Pure registry: only keys successfully pressed by this macro are tracked.
#[derive(Default, Debug)]
struct KeyRegistry(Vec<u16>);
impl KeyRegistry {
    fn press(&mut self, vk: u16) -> bool { if self.0.contains(&vk) { false } else { self.0.push(vk); true } }
    fn release(&mut self, vk: u16) -> bool { if let Some(i) = self.0.iter().position(|&key| key == vk) { self.0.remove(i); true } else { false } }
    fn release_all(&mut self) -> Vec<u16> { self.0.drain(..).rev().collect() }
}
struct HoldCleanup(Arc<std::sync::Mutex<KeyRegistry>>);
impl Drop for HoldCleanup { fn drop(&mut self) { let _ = release_held(&self.0); } }
fn release_held(held: &std::sync::Mutex<KeyRegistry>) -> Result<()> {
    let mut held = held.lock().unwrap_or_else(|e| e.into_inner());
    let mut first_error = None;
    for vk in held.release_all() {
        if let Err(err) = input(vk, 0, KEYEVENTF_KEYUP) { if first_error.is_none() { first_error = Some(err); } }
    }
    if let Some(err) = first_error { Err(err.context("Relâchement des touches maintenues impossible")) } else { Ok(()) }
}
fn hold_keys(keys: &[u16], action: &str, held: &std::sync::Mutex<KeyRegistry>, cancel: &AtomicBool) -> Result<()> {
    let mut held = held.lock().unwrap_or_else(|e| e.into_inner());
    check(cancel)?;
    match action {
        "down" => for &vk in keys {
            if !held.0.contains(&vk) { input(vk, 0, 0).context("Maintien de touche refusé par Windows")?; held.press(vk); }
        },
        "up" => for &vk in keys.iter().rev() {
            if held.0.contains(&vk) { input(vk, 0, KEYEVENTF_KEYUP).context("Relâchement de touche refusé par Windows")?; held.release(vk); }
        },
        _ => bail!("Action de maintien inconnue"),
    }
    Ok(())
}
const MAX_PENDING: usize = 5;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PressDecision { Start, Ignore, Debounced, Restart, Queue, Full }
fn press_decision(mode: Option<crate::model::RunMode>, running: bool, pending: usize, elapsed_ms: Option<u64>, debounce_ms: u32) -> PressDecision {
    if debounce_ms > 0 && elapsed_ms.is_some_and(|ms| ms < u64::from(debounce_ms)) { return PressDecision::Debounced; }
    if !running { return PressDecision::Start; }
    match mode.unwrap_or_default() {
        crate::model::RunMode::Ignore => PressDecision::Ignore,
        crate::model::RunMode::Restart => PressDecision::Restart,
        crate::model::RunMode::Queue if pending < MAX_PENDING => PressDecision::Queue,
        crate::model::RunMode::Queue => PressDecision::Full,
    }
}
type Job = (Key, bool, u64, crate::model::ObsSettings, bool);
#[derive(Default)]
struct ExecutionState {
    queue: std::collections::VecDeque<Job>,
    presses: std::collections::HashMap<String, Instant>,
}
pub struct Engine {
    pub running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    held: Arc<std::sync::Mutex<KeyRegistry>>,
    tx: Sender<Event>,
    last_finished: Arc<std::sync::Mutex<Instant>>,
    state: Arc<std::sync::Mutex<ExecutionState>>,
}
impl Engine {
    pub fn new(tx: Sender<Event>) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(AtomicBool::new(false)),
            held: Arc::new(std::sync::Mutex::new(KeyRegistry::default())),
            tx,
            last_finished: Arc::new(std::sync::Mutex::new(Instant::now() - Duration::from_secs(1))),
            state: Arc::new(std::sync::Mutex::new(ExecutionState::default())),
        }
    }
    pub fn stop(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.queue.clear();
        self.cancel.store(true, Ordering::SeqCst);
        drop(state);
        if let Err(e) = release_held(&self.held) { let _ = self.tx.send(Event::Log(format!("{e:#}"))); }
    }
    pub fn allows_global_press(&self, at: Instant) -> bool {
        at > *self.last_finished.lock().unwrap_or_else(|e| e.into_inner()) + Duration::from_millis(150)
    }
    pub fn start(&self, key: Key, dry: bool, initial_delay: u64, obs: crate::model::ObsSettings) {
        self.start_inner(key, dry, initial_delay, obs, false);
    }
    pub fn start_workshop(&self, key: Key, dry: bool, obs: crate::model::ObsSettings) {
        self.start_inner(key, dry, 0, obs, true);
    }
    fn start_inner(&self, key: Key, dry: bool, initial_delay: u64, obs: crate::model::ObsSettings, workshop: bool) {
        let now = Instant::now();
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let id = key.runtime_id.clone().unwrap_or_else(|| serde_json::to_string(&key).unwrap_or_default());
        let elapsed = state.presses.get(&id).map(|t| now.saturating_duration_since(*t).as_millis() as u64);
        let decision = press_decision(key.run_mode, self.running.load(Ordering::SeqCst), state.queue.len(), elapsed, key.debounce_ms);
        match decision {
            PressDecision::Ignore => { let _ = self.tx.send(Event::Log("Macro déjà en cours : nouvel appui ignoré".into())); return; }
            PressDecision::Debounced => { let _ = self.tx.send(Event::Log("Appui ignoré : anti-rebond actif".into())); return; }
            PressDecision::Full => { let _ = self.tx.send(Event::Log("File d'attente pleine (5 macros)".into())); return; }
            _ => {}
        }
        if state.presses.len() > 2048 { state.presses.clear(); }
        state.presses.insert(id, now);
        if decision == PressDecision::Restart {
            state.queue.clear();
            self.cancel.store(true, Ordering::SeqCst);
            state.queue.push_back((key, dry, initial_delay, obs, workshop));
            let _ = self.tx.send(Event::Log("Macro relancée après interruption".into()));
            return;
        }
        if decision == PressDecision::Queue {
            state.queue.push_back((key, dry, initial_delay, obs, workshop));
            let _ = self.tx.send(Event::Log(format!("Macro en attente ({}/5)", state.queue.len())));
            return;
        }
        self.running.store(true, Ordering::SeqCst);
        self.cancel.store(false, Ordering::SeqCst);
        drop(state);
        let running = self.running.clone();
        let cancel = self.cancel.clone();
        let held = self.held.clone();
        let tx = self.tx.clone();
        let last_finished = self.last_finished.clone();
        let state = self.state.clone();
        std::thread::spawn(move || {
            let mut job = (key, dry, initial_delay, obs, workshop);
            loop {
                let (key, dry, initial_delay, obs, workshop) = job;
                WORKSHOP_RUN.with(|flag| flag.set(workshop));
                let run_started = Instant::now();
                // Physical press: capture before spawning; delayed test: capture after countdown.
                let mouse_window = (!dry && initial_delay == 0).then(capture_mouse_window);
                let captured = if !dry && initial_delay == 0 && has_app_volume(&key.steps) {
                    Some(crate::app_audio::capture("foreground", true))
                } else { None };
                let cleanup = (!dry).then(|| HoldCleanup(held.clone()));
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if !dry { wait(initial_delay, &cancel)?; }
                    MOUSE_WINDOW.with(|area| area.set(if dry { None } else { mouse_window.unwrap_or_else(capture_mouse_window) }));
                    ACTIVE_KEY.with(|id| *id.borrow_mut() = key.runtime_id.clone().unwrap_or_else(|| serde_json::to_string(&key).unwrap_or_default()));
                    let foreground = if dry || !has_app_volume(&key.steps) { None }
                        else if let Some(captured) = captured { Some(captured?) }
                        else { Some(crate::app_audio::capture("foreground", true)?) };
                    run(&key, dry, &cancel, &tx, &obs, foreground.as_ref(), &held)
                }));
                drop(cleanup);
                let message = match result {
                    Ok(Ok(())) => "Macro terminée".into(),
                    Ok(Err(e)) => format!("Macro arrêtée : {e}"),
                    Err(_) => "Erreur interne : macro interrompue".into(),
                };
                if workshop { let _ = tx.send(Event::WorkshopFinished { ok: message == "Macro terminée", elapsed: run_started.elapsed(), message: message.clone() }); }
                let _ = tx.send(Event::Log(message));
                let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
                if cancel.load(Ordering::SeqCst) && guard.queue.is_empty() {
                    *last_finished.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
                    running.store(false, Ordering::SeqCst);
                    break;
                }
                if let Some(next) = guard.queue.pop_front() {
                    cancel.store(false, Ordering::SeqCst);
                    job = next;
                } else {
                    *last_finished.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
                    running.store(false, Ordering::SeqCst);
                    break;
                }
            }
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
pub(crate) fn check(cancel: &AtomicBool) -> Result<()> {
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
fn has_app_volume(steps: &[Step]) -> bool {
    steps.iter().any(|s| match s.active() {
        Step::AppVolume { target, .. } => target == "foreground",
        Step::Repeat { steps, .. } => has_app_volume(steps),
        Step::Toggle { a, b } => has_app_volume(a) || has_app_volume(b),
        Step::IfWindow { then_steps, else_steps, .. } | Step::IfVariable { then_steps, else_steps, .. } | Step::IfCondition { then_steps, else_steps, .. } => has_app_volume(then_steps) || has_app_volume(else_steps),
        _ => false,
    })
}
fn run(key: &Key, dry: bool, cancel: &AtomicBool, tx: &Sender<Event>, obs: &crate::model::ObsSettings, foreground: Option<&crate::app_audio::Target>, held: &std::sync::Mutex<KeyRegistry>) -> Result<()> {
    let _ = tx.send(Event::Log(format!(
        "{} — {} ({} répétition(s))",
        if dry { "SIMULATION" } else { "EXÉCUTION" },
        key.label,
        key.repeat
    )));
    let mut variables = crate::persistent_variables::snapshot();
    let mut budget = 10_000usize;
    let mut saved_clipboard = None;
    for _ in 0..if dry { 1 } else { key.repeat } {
        run_steps(
            &key.steps,
            dry,
            cancel,
            tx,
            &mut variables,
            &mut budget,
            &mut saved_clipboard,
            &[],
            obs,
            foreground,
            held,
        )?;
    }
    let _ = tx.send(Event::Block(vec![]));
    Ok(())
}
// Les gabarits restent intacts dans la configuration. L'horloge et le presse-papiers
// ne sont consultés qu'à l'exécution ; cette fonction est pure pour les tests.
fn interpolate_special(text: &str, vars: &std::collections::BTreeMap<String, String>, date: &str, heure: &str, timestamp: u64, clipboard: &str) -> String {
    let mut values = vars.clone();
    values.insert("date".into(), date.into());
    values.insert("heure".into(), heure.into());
    values.insert("horodatage".into(), timestamp.to_string());
    values.insert("presse_papiers".into(), clipboard.into());
    interpolate(text, &values)
}

fn clipboard_text(cancel: &AtomicBool) -> Result<String> {
    use windows_sys::Win32::System::{DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard}, Memory::{GlobalLock, GlobalSize, GlobalUnlock}};
    for attempt in 0..5 {
        check(cancel)?;
        if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
            let result = (|| {
                let handle = unsafe { GetClipboardData(13) /* CF_UNICODETEXT */ };
                if handle.is_null() { return Err(anyhow::anyhow!("Presse-papiers : texte Unicode indisponible")); }
                let bytes = unsafe { GlobalSize(handle) };
                if bytes == 0 || bytes > 200_000 { bail!("Presse-papiers : texte vide ou trop long"); }
                let pointer = unsafe { GlobalLock(handle) };
                if pointer.is_null() { bail!("Presse-papiers : lecture impossible"); }
                let words = unsafe { std::slice::from_raw_parts(pointer.cast::<u16>(), bytes / 2) };
                let end = words.iter().position(|&w| w == 0).unwrap_or(words.len());
                let text = String::from_utf16(&words[..end]).context("Presse-papiers : texte Unicode invalide");
                unsafe { GlobalUnlock(handle); }
                text
            })();
            unsafe { CloseClipboard(); }
            return result;
        }
        if attempt < 4 { wait(20, cancel)?; }
    }
    bail!("Presse-papiers occupé : lecture impossible après plusieurs essais")
}

fn clipboard_set(text: &str, cancel: &AtomicBool) -> Result<()> {
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::{DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData}, Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE}};
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    anyhow::ensure!(wide.len() <= 100_001, "Presse-papiers : texte trop long");
    for attempt in 0..5 {
        check(cancel)?;
        if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
            let result = (|| {
                let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) };
                if handle.is_null() { bail!("Presse-papiers : allocation impossible"); }
                let pointer = unsafe { GlobalLock(handle) };
                if pointer.is_null() {
                    unsafe { GlobalFree(handle); }
                    bail!("Presse-papiers : verrouillage mémoire impossible");
                }
                unsafe { std::ptr::copy_nonoverlapping(wide.as_ptr(), pointer.cast::<u16>(), wide.len()); GlobalUnlock(handle); }
                if unsafe { EmptyClipboard() } == 0 {
                    unsafe { GlobalFree(handle); }
                    bail!("Presse-papiers : vidage impossible");
                }
                if unsafe { SetClipboardData(13, handle) }.is_null() {
                    unsafe { GlobalFree(handle); }
                    bail!("Presse-papiers : écriture Unicode impossible");
                }
                Ok(())
            })();
            unsafe { CloseClipboard(); }
            return result;
        }
        if attempt < 4 { wait(20, cancel)?; }
    }
    bail!("Presse-papiers occupé : écriture impossible après plusieurs essais")
}

fn interpolate_runtime(text: &str, vars: &std::collections::BTreeMap<String, String>, cancel: &AtomicBool) -> Result<String> {
    check(cancel)?;
    let mut now = std::mem::MaybeUninit::uninit();
    unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(now.as_mut_ptr()); }
    let now = unsafe { now.assume_init() };
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .context("Horloge système indisponible")?.as_secs();
    let clipboard = if text.contains("{presse_papiers}") { clipboard_text(cancel)? } else { String::new() };
    Ok(interpolate_special(text, vars, &format!("{:04}-{:02}-{:02}", now.wYear, now.wMonth, now.wDay),
        &format!("{:02}:{:02}:{:02}", now.wHour, now.wMinute, now.wSecond), timestamp, &clipboard))
}

#[cfg(test)]
mod execution_decision_tests {
    use super::*;
    use crate::model::RunMode;
    #[test]
    fn new_press_while_running_and_debounce() {
        use PressDecision::*;
        assert_eq!(press_decision(None, false, 0, None, 0), Start);
        assert_eq!(press_decision(None, true, 0, None, 0), Ignore);
        assert_eq!(press_decision(Some(RunMode::Restart), true, 2, Some(100), 0), Restart);
        assert_eq!(press_decision(Some(RunMode::Queue), true, 4, Some(100), 0), Queue);
        assert_eq!(press_decision(Some(RunMode::Queue), true, 5, Some(100), 0), Full);
        assert_eq!(press_decision(Some(RunMode::Queue), true, 0, Some(99), 100), Debounced);
        assert_eq!(press_decision(Some(RunMode::Queue), true, 0, Some(100), 100), Queue);
        assert_eq!(press_decision(Some(RunMode::Restart), true, 0, Some(0), 100), Debounced);
    }
}
#[cfg(test)]
mod interpolation_tests {
    use super::*;
    #[test]
    fn specials_are_pure_and_escape_and_preserve_unknown_placeholders() {
        let vars = std::collections::BTreeMap::from([("nom".into(), "Zoé".into()), ("date".into(), "faux".into())]);
        assert_eq!(interpolate_special("{nom} {date} {heure} {horodatage} {presse_papiers} {{date} {inconnue}",
            &vars, "2026-10-17", "09:08:07", 1792228087, "été"), "Zoé 2026-10-17 09:08:07 1792228087 été {date} {inconnue}");
    }
}

pub(crate) fn interpolate(text: &str, vars: &std::collections::BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '{' { out.push(c); continue; }
        if chars.peek() == Some(&'{') { chars.next(); out.push('{'); continue; }
        let mut key = String::new();
        let mut closed = false;
        for next in chars.by_ref() {
            if next == '}' { closed = true; break; }
            key.push(next);
        }
        if closed { if let Some(value) = vars.get(&key) { out.push_str(value); } else { out.push('{'); out.push_str(&key); out.push('}'); } }
        else { out.push('{'); out.push_str(&key); }
    }
    out
}

#[cfg(test)]
mod math_tests {
    use super::*;
    #[test]
    fn arithmetic_and_errors_are_pure() {
        assert_eq!(calculate(2.0, "add", 1.0, 0.0, 10.0, 0.0).unwrap(), 3.0);
        assert_eq!(calculate(2.0, "subtract", 1.0, 0.0, 10.0, 0.0).unwrap(), 1.0);
        assert_eq!(calculate(8.0, "divide", 2.0, 0.0, 10.0, 0.0).unwrap(), 4.0);
        assert_eq!(calculate(9.0, "modulo", 4.0, 0.0, 10.0, 0.0).unwrap(), 1.0);
        assert_eq!(calculate(4.0, "min", 2.0, 0.0, 10.0, 0.0).unwrap(), 2.0);
        assert_eq!(calculate(4.0, "max", 8.0, 0.0, 10.0, 0.0).unwrap(), 8.0);
        assert_eq!(calculate(0.0, "random", 0.0, 2.0, 6.0, 0.5).unwrap(), 4.0);
        assert!(calculate(1.0, "divide", 0.0, 0.0, 10.0, 0.0).unwrap_err().to_string().contains("zéro"));
        assert!(calculate(f64::MAX, "multiply", 2.0, 0.0, 10.0, 0.0).is_err());
    }
}

#[cfg(test)]
mod key_registry_tests {
    use super::*;
    #[test]
    fn registry_is_idempotent_and_cleans_up_in_reverse_order() {
        let mut held = KeyRegistry::default();
        assert!(held.press(0x11));
        assert!(held.press(0x41));
        assert!(!held.press(0x11));
        assert!(!held.release(0x42));
        assert!(held.release(0x41));
        assert!(held.press(0x42));
        assert_eq!(held.release_all(), vec![0x42, 0x11]);
        assert!(held.release_all().is_empty());
    }
}
fn calculate(current: f64, op: &str, operand: f64, min: f64, max: f64, random_unit: f64) -> Result<f64> {
    anyhow::ensure!(current.is_finite() && operand.is_finite() && min.is_finite() && max.is_finite(), "Calcul : nombre non fini");
    let result = match op {
        "set" => operand, "add" => current + operand, "subtract" => current - operand,
        "multiply" => current * operand,
        "divide" => { anyhow::ensure!(operand != 0.0, "Calcul : division par zéro"); current / operand },
        "modulo" => { anyhow::ensure!(operand != 0.0, "Calcul : modulo par zéro"); current % operand },
        "min" => current.min(operand), "max" => current.max(operand),
        "random" => { anyhow::ensure!(min <= max && (0.0..1.0).contains(&random_unit), "Calcul : intervalle aléatoire invalide"); min + (max - min) * random_unit },
        _ => bail!("Calcul : opération inconnue"),
    };
    anyhow::ensure!(result.is_finite(), "Calcul : résultat hors limites");
    Ok(result)
}

fn text_operation(current: &str, op: &str, value: &str, find: &str) -> Result<String> {
    let result = match op {
        "set" => value.to_owned(),
        "append" => format!("{current}{value}"),
        "prepend" => format!("{value}{current}"),
        "upper" => current.to_uppercase(),
        "lower" => current.to_lowercase(),
        "trim" => current.trim().to_owned(),
        "replace" if !find.is_empty() => current.replace(find, value),
        "length" => current.chars().count().to_string(),
        _ => anyhow::bail!("Texte : opération ou recherche invalide"),
    };
    anyhow::ensure!(result.len() <= 100_000, "Texte : résultat trop long (100 000 octets maximum)");
    Ok(result)
}

#[cfg(test)]
mod text_op_tests {
    use super::*;
    #[test]
    fn operations_are_pure_and_unicode_counts_characters() {
        assert_eq!(text_operation("déjà", "upper", "", "").unwrap(), "DÉJÀ");
        assert_eq!(text_operation("AbC", "lower", "", "").unwrap(), "abc");
        assert_eq!(text_operation("  oui  ", "trim", "", "").unwrap(), "oui");
        assert_eq!(text_operation("abc", "set", "é", "").unwrap(), "é");
        assert_eq!(text_operation("a", "append", "b", "").unwrap(), "ab");
        assert_eq!(text_operation("a", "prepend", "b", "").unwrap(), "ba");
        assert_eq!(text_operation("aba", "replace", "é", "a").unwrap(), "ébé");
        assert_eq!(text_operation("é🙂", "length", "", "").unwrap(), "2");
        assert!(text_operation("abc", "replace", "x", "").is_err());
        assert!(text_operation(&"a".repeat(100_000), "append", "b", "").is_err());
    }
    #[test]
    fn interpolation_precedes_operations() {
        let vars = std::collections::BTreeMap::from([("mot".into(), "école".into())]);
        let value = interpolate("{mot}", &vars);
        assert_eq!(text_operation("", "upper", &value, "").unwrap(), "");
        assert_eq!(text_operation("", "set", &value, "").unwrap(), "école");
    }
}

fn run_steps(
    steps: &[Step],
    dry: bool,
    cancel: &AtomicBool,
    tx: &Sender<Event>,
    variables: &mut std::collections::BTreeMap<String, String>,
    budget: &mut usize,
    saved_clipboard: &mut Option<String>,
    parent: &[usize],
    obs: &crate::model::ObsSettings,
    foreground: Option<&crate::app_audio::Target>,
    held: &std::sync::Mutex<KeyRegistry>,
) -> Result<()> {
    for (idx, step) in steps.iter().enumerate() {
        if step.is_disabled() { continue; }
        check(cancel)?;
        anyhow::ensure!(*budget > 0, "Limite de 10 000 blocs exécutés atteinte");
        *budget -= 1;
        let mut path = parent.to_vec();
        path.push(idx);
        let started = Instant::now();
        if WORKSHOP_RUN.with(|flag| flag.get()) { let _ = tx.send(Event::WorkshopStep { path: path.clone(), name: step.name().into(), elapsed: None }); }
        let _ = tx.send(Event::Block(path.clone()));
        let _ = tx.send(Event::Log(format!(
            "Bloc {} : {}",
            path.iter()
                .map(|i| (i + 1).to_string())
                .collect::<Vec<_>>()
                .join("."),
            step.name()
        )));
        if dry {
            if let Step::Notify { title, message } = step { let _ = tx.send(Event::Log(format!("Simulation : notification « {} » — {} (non envoyée)", interpolate(title, variables), interpolate(message, variables)))); }
            if let Some(summary) = step.active().app_volume_summary().or_else(|| step.active().device_volume_summary()) { let _ = tx.send(Event::Log(format!("Simulation : {summary}"))); }
            match step {
                Step::WindowOp { .. } => { let _ = tx.send(Event::Log(format!("Simulation : {} (aucune fenêtre modifiée)", step.window_op_summary().unwrap_or_default()))); }
                Step::KeyHold { .. } => { let _ = tx.send(Event::Log(format!("Simulation : {} (aucune touche enfoncée)", step.key_hold_summary().unwrap_or_default()))); }
                Step::Clipboard { .. } => {
                    let _ = tx.send(Event::Log(format!("Simulation : {} (presse-papiers non consulté ni modifié)", step.clipboard_summary().unwrap_or_default())));
                }
                Step::TextOp { .. } => {
                    let _ = tx.send(Event::Log(format!("Simulation : {} (aucune variable modifiée)", step.text_op_summary().unwrap_or_default())));
                }
                Step::Math { name, op, value, min, max, .. } => {
                    let _ = tx.send(Event::Log(format!("Simulation : {} (aucune variable modifiée)", step.math_summary().unwrap_or_default())));
                    let _ = (name, op, value, min, max);
                }
                Step::WaitUntil { condition, .. } => {
                    let result = condition.evaluate(variables)?;
                    let _ = tx.send(Event::Log(format!("Simulation : {} = {result} ; aucune attente ni action", condition.summary())));
                }
                Step::IfCondition { condition, then_steps, else_steps } => {
                    let result = condition.evaluate(variables)?;
                    let _ = tx.send(Event::Log(format!("Simulation : {} = {result} ; ALORS et SINON explorés sans effet", condition.summary())));
                    for (branch_id, branch) in [then_steps, else_steps].into_iter().enumerate() {
                        run_steps(branch, true, cancel, tx, variables, budget, saved_clipboard, &[path.clone(), vec![branch_id]].concat(), obs, foreground, held)?;
                    }
                }
                Step::IfWindow {
                    then_steps,
                    else_steps,
                    ..
                }
                | Step::IfVariable {
                    then_steps,
                    else_steps,
                    ..
                } => {
                    run_steps(
                        then_steps,
                        true,
                        cancel,
                        tx,
                        variables,
                        budget,
                    saved_clipboard,
                        &[path.clone(), vec![0]].concat(),
                        obs,
                        foreground,
                        held,
                    )?;
                    run_steps(
                        else_steps,
                        true,
                        cancel,
                        tx,
                        variables,
                        budget,
                    saved_clipboard,
                        &[path.clone(), vec![1]].concat(),
                        obs,
                        foreground,
                        held,
                    )?;
                }
                Step::Toggle { a, b } => {
                    for (branch, list) in [a, b].into_iter().enumerate() {
                        run_steps(list, true, cancel, tx, variables, budget, saved_clipboard, &[path.clone(), vec![branch]].concat(), obs, foreground, held)?;
                    }
                },
                Step::Repeat { steps, .. } => run_steps(
                    steps,
                    true,
                    cancel,
                    tx,
                    variables,
                    budget,
                    saved_clipboard,
                    &[path.clone(), vec![0]].concat(),
                    obs,
                    foreground,
                    held,
                )?,
                _ => {}
            }
            if WORKSHOP_RUN.with(|flag| flag.get()) { let _ = tx.send(Event::WorkshopStep { path, name: step.name().into(), elapsed: Some(started.elapsed()) }); }
            continue;
        }
        match step {
            Step::Disabled { .. } => {},
            Step::Toggle { a, b } => {
                let branch = next_branch(&path);
                let _ = tx.send(Event::ToggleVisual);
                run_steps(if branch == 0 { a } else { b }, dry, cancel, tx, variables, budget, saved_clipboard, &[path.clone(), vec![branch]].concat(), obs, foreground, held)?;
            },
            Step::PlaySound { path, volume } => crate::soundboard::play(path, *volume)?,
            Step::StopSounds => crate::soundboard::stop(),
            Step::UdpSend { host, port, text } => crate::network_actions::udp_send(host, *port, &interpolate_runtime(text, variables, cancel)?)?,
            Step::HttpRequest { method, url, body, content_type } => { let url = interpolate_runtime(url, variables, cancel)?; let body = interpolate_runtime(body, variables, cancel)?; crate::model::validate_http(method, &url, &body, content_type)?; let reply = crate::network_actions::http_request(method, &url, &body, content_type)?; let _ = tx.send(Event::Log(reply)); },
            Step::D6Brightness { mode, value } => { let _ = tx.send(Event::D6Brightness(mode.clone(), *value)); },
            Step::D6Sleep => { let _ = tx.send(Event::D6Sleep); },
            Step::CloseWindow => close_foreground_window()?,
            Step::WindowOp { target, value, op, x, y, w, h, monitor } => {
                let message = window_operation(target, value, op, *x, *y, *w, *h, *monitor, cancel)?;
                let _ = tx.send(Event::Log(message));
            },
            Step::MouseClick { button, double, x, y, relative_to } => {
                if let (Some(x), Some(y)) = (x, y) { mouse_move(*x, *y, relative_to)?; }
                mouse_click(button, *double)?;
            },
            Step::MouseMove { x, y, relative, relative_to } => mouse_move(*x, *y, if *relative { "cursor" } else { relative_to })?,
            Step::MouseScroll { amount, horizontal } => mouse_event(if *horizontal { MOUSEEVENTF_HWHEEL } else { MOUSEEVENTF_WHEEL }, amount.wrapping_mul(120) as u32, 0, 0)?,
            Step::ObsScene { .. } | Step::ObsRecord { .. } | Step::ObsStream { .. }
            | Step::ObsVirtualCam { .. } | Step::ObsReplay { .. } | Step::ObsMute { .. }
            | Step::ObsSourceVisibility { .. } => crate::obs::execute(step, obs)?,
            Step::DeviceVolume { input, device, action, step, level } => {
                let message = crate::audio::device_volume(device, *input, action, *step, *level)?;
                let _ = tx.send(Event::Log(message));
            }
            Step::AppVolume { target, action, step, level, include_children } => {
                let selected = if target == "foreground" {
                    foreground.context("Cible au premier plan non capturée")?.clone()
                } else { crate::app_audio::capture(target, *include_children)? };
                let message = crate::app_audio::apply(&selected, action, *step, *level)?;
                let _ = tx.send(Event::Log(message));
            }
            Step::AudioSwitch {
                first,
                second,
                input,
                ..
            } => {
                crate::audio::toggle(first, second, *input)?;
            }
            Step::Notify { title, message } => {
                let title = interpolate(title, variables);
                let message = interpolate(message, variables);
                let _ = tx.send(Event::Notification(title, message));
            }
            Step::TextOp { name, op, value, find, persistent } => {
                let current = variables.get(name).map(String::as_str).unwrap_or("");
                let value = interpolate(value, variables);
                let find = interpolate(find, variables);
                let result = text_operation(current, op, &value, &find)?;
                if *persistent { crate::persistent_variables::set(name, &result)?; }
                variables.insert(name.clone(), result.clone());
                let _ = tx.send(Event::Log(format!("Texte : {name} = {} caractères", result.chars().count())));
            }
            Step::Math { name, op, value, min, max, persistent } => {
                let current = variables.get(name).map(|s| s.parse::<f64>()).transpose()
                    .map_err(|_| anyhow::anyhow!("Calcul : variable « {name} » non numérique"))?.unwrap_or(0.0);
                let operand = if op == "random" { 0.0 } else { interpolate(value, variables).trim().parse::<f64>()
                    .map_err(|_| anyhow::anyhow!("Calcul : valeur numérique invalide pour « {name} »"))? };
                let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| anyhow::anyhow!("Horloge système indisponible"))?.subsec_nanos();
                let result = calculate(current, op, operand, *min, *max, f64::from(nanos) / 1_000_000_000.0)?;
                let value = result.to_string();
                if *persistent { crate::persistent_variables::set(name, &value)?; }
                variables.insert(name.clone(), value.clone());
                let _ = tx.send(Event::Log(format!("Calcul : {name} = {value}")));
            }
            Step::Clipboard { op, value, name } => {
                match op.as_str() {
                    "copy_text" => clipboard_set(&interpolate(value, variables), cancel)?,
                    "read_to_variable" => { variables.insert(name.clone(), clipboard_text(cancel)?); }
                    "clear" => clipboard_set("", cancel)?,
                    "save" => { *saved_clipboard = Some(clipboard_text(cancel)?); }
                    "restore" => clipboard_set(saved_clipboard.as_deref().ok_or_else(|| anyhow::anyhow!("Presse-papiers : aucun texte sauvegardé dans cette macro"))?, cancel)?,
                    _ => bail!("Presse-papiers : opération inconnue"),
                }
                let _ = tx.send(Event::Log(format!("Presse-papiers : {}", step.clipboard_summary().unwrap_or_default())));
            }
            Step::SetVariable { name, value, persistent } => {
                let value = interpolate(value, variables);
                if *persistent { crate::persistent_variables::set(name, &value)?; }
                variables.insert(name.clone(), value);
            }
            Step::WaitUntil { condition, timeout_ms, poll_ms } => {
                let start = Instant::now();
                loop {
                    check(cancel)?;
                    if condition.evaluate(variables)? {
                        let _ = tx.send(Event::Log(format!("Condition atteinte : {}", condition.summary())));
                        break;
                    }
                    let remaining = Duration::from_millis(*timeout_ms).saturating_sub(start.elapsed());
                    anyhow::ensure!(!remaining.is_zero(), "Délai expiré après {timeout_ms} ms : {}", condition.summary());
                    wait(remaining.min(Duration::from_millis(*poll_ms)).as_millis() as u64, cancel)?;
                }
            }
            Step::IfCondition { condition, then_steps, else_steps } => {
                let result = condition.evaluate(variables)?;
                let _ = tx.send(Event::Log(format!("{} : {}", condition.summary(), if result { "ALORS" } else { "SINON" })));
                let branch_id = usize::from(!result);
                run_steps(if result { then_steps } else { else_steps }, false, cancel, tx, variables, budget, saved_clipboard, &[path.clone(), vec![branch_id]].concat(), obs, foreground, held)?;
            }
            Step::IfWindow {
                title,
                then_steps,
                else_steps,
            } => {
                let (branch, branch_id) = if find_window(title).is_some() {
                    (then_steps, 0)
                } else {
                    (else_steps, 1)
                };
                run_steps(
                    branch,
                    dry,
                    cancel,
                    tx,
                    variables,
                    budget,
                    saved_clipboard,
                    &[path.clone(), vec![branch_id]].concat(),
                    obs,
                    foreground,
                    held,
                )?;
            }
            Step::IfVariable {
                name,
                value,
                then_steps,
                else_steps,
            } => {
                let (branch, branch_id) = if variables.get(name) == Some(value) {
                    (then_steps, 0)
                } else {
                    (else_steps, 1)
                };
                run_steps(
                    branch,
                    dry,
                    cancel,
                    tx,
                    variables,
                    budget,
                    saved_clipboard,
                    &[path.clone(), vec![branch_id]].concat(),
                    obs,
                    foreground,
                    held,
                )?;
            }
            Step::Repeat { count, steps } => {
                for _ in 0..*count {
                    run_steps(
                        steps,
                        dry,
                        cancel,
                        tx,
                        variables,
                        budget,
                    saved_clipboard,
                        &[path.clone(), vec![0]].concat(),
                        obs,
                        foreground,
                        held,
                    )?;
                }
            }
            Step::AutoHotkey {
                executable,
                script,
                args,
            } => crate::automation::run_script(executable, script, args, cancel)?,
            Step::Delay { ms } => wait(*ms, cancel)?,
            Step::Hotkey { keys } => hotkey(&parse_hotkey(keys)?)?,
            Step::KeyHold { keys, action, hold_ms } => {
                let codes = parse_hotkey(keys)?;
                hold_keys(&codes, action, held, cancel)?;
                if action == "down" && *hold_ms > 0 { wait(*hold_ms, cancel)?; hold_keys(&codes, "up", held, cancel)?; }
            }
            Step::Text { text } => {
                for unit in interpolate_runtime(text, variables, cancel)?.encode_utf16() {
                    check(cancel)?;
                    input(0, unit, KEYEVENTF_UNICODE)?;
                    input(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)?;
                }
            }
            Step::Launch { program, args, focus_if_running } => {
                if *focus_if_running {
                    if let Some(window) = find_window_by_exe(program) {
                        activate_window(window)?;
                        let _ = tx.send(Event::Log(format!("Application déjà ouverte : {program} activée")));
                        continue;
                    }
                }
                let args = args.iter().map(|arg| interpolate_runtime(arg, variables, cancel)).collect::<Result<Vec<_>>>()?;
                let mut child = std::process::Command::new(resolve_program(program)).args(&args).spawn()?;
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Step::Url { url } => {
                let url = interpolate_runtime(url, variables, cancel)?;
                anyhow::ensure!((url.starts_with("http://") || url.starts_with("https://")) && !url.chars().any(char::is_control), "Adresse HTTP/HTTPS interpolée invalide");
                let mut child = std::process::Command::new("explorer.exe")
                    .arg(&url)
                    .spawn()?;
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Step::Media { command } => hotkey(&[
                media_vk(command).ok_or_else(|| anyhow::anyhow!("Commande média invalide"))?
            ])?,
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

        if WORKSHOP_RUN.with(|flag| flag.get()) { let _ = tx.send(Event::WorkshopStep { path, name: step.name().into(), elapsed: Some(started.elapsed()) }); }
        wait(10, cancel)?;
    }
    Ok(())
}
fn mouse_event(flags: u32, data: u32, dx: i32, dy: i32) -> Result<()> {
    let event = INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: MOUSEINPUT {
        dx, dy, mouseData: data, dwFlags: flags, time: 0, dwExtraInfo: 0,
    } } };
    anyhow::ensure!(unsafe { SendInput(1, &event, std::mem::size_of::<INPUT>() as i32) } == 1, "Windows a refusé l'action souris");
    Ok(())
}
fn mouse_click(button: &str, double: bool) -> Result<()> {
    let (down, up) = match button {
        "left" => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        "right" => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        "middle" => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
        _ => anyhow::bail!("Bouton souris invalide"),
    };
    for _ in 0..if double { 2 } else { 1 } {
        mouse_event(down, 0, 0, 0)?;
        mouse_event(up, 0, 0, 0)?;
    }
    Ok(())
}
// Capturée au déclenchement (ou après le compte à rebours du test), jamais à chaque étape.
fn capture_mouse_window() -> Option<(i32, i32, i32, i32)> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() { return None; }
    let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    let mut origin = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
    if unsafe { GetClientRect(hwnd, &mut rect) } == 0 || unsafe { windows_sys::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut origin) } == 0 { return None; }
    Some((origin.x, origin.y, rect.right - rect.left, rect.bottom - rect.top))
}
// Pure : borne aux pixels du bureau virtuel, puis convertit vers SendInput.
fn mouse_target(x: i32, y: i32, origin: &str, cursor: (i32, i32), window: Option<(i32, i32, i32, i32)>, screen: (i32, i32, i32, i32)) -> Result<(i32, i32)> {
    let (sx, sy, sw, sh) = screen;
    anyhow::ensure!(sw > 1 && sh > 1, "Écran introuvable");
    let (base_x, base_y) = match origin {
        "screen" => (0, 0),
        "cursor" => cursor,
        "window" => {
            let (wx, wy, w, h) = window.context("Fenêtre au premier plan non capturée au déclenchement")?;
            anyhow::ensure!(w > 0 && h > 0 && x >= 0 && y >= 0 && x < w && y < h, "Position hors de la zone client de la fenêtre");
            (wx, wy)
        },
        _ => bail!("Origine de souris inconnue"),
    };
    let px = (i64::from(base_x) + i64::from(x)).clamp(i64::from(sx), i64::from(sx) + i64::from(sw) - 1);
    let py = (i64::from(base_y) + i64::from(y)).clamp(i64::from(sy), i64::from(sy) + i64::from(sh) - 1);
    Ok((((px - i64::from(sx)) * 65535 / i64::from(sw - 1)) as i32,
        ((py - i64::from(sy)) * 65535 / i64::from(sh - 1)) as i32))
}
fn mouse_move(x: i32, y: i32, relative_to: &str) -> Result<()> {
    let mut cursor = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
    anyhow::ensure!(unsafe { GetCursorPos(&mut cursor) } != 0, "Position du curseur indisponible");
    let screen = unsafe { (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN), GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN)) };
    let window = MOUSE_WINDOW.with(|area| area.get());
    let (dx, dy) = mouse_target(x, y, relative_to, (cursor.x, cursor.y), window, screen)?;
    // Les bornes du bureau virtuel peuvent inclure des trous entre écrans.
    let (sx, sy, sw, sh) = screen;
    let px = i64::from(sx) + i64::from(dx) * i64::from(sw - 1) / 65535;
    let py = i64::from(sy) + i64::from(dy) * i64::from(sh - 1) / 65535;
    let pt = windows_sys::Win32::Foundation::POINT { x: px as i32, y: py as i32 };
    let monitor = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
    anyhow::ensure!(!monitor.is_null(), "Aucun écran réel disponible");
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        rcMonitor: RECT { left: 0, top: 0, right: 0, bottom: 0 },
        rcWork: RECT { left: 0, top: 0, right: 0, bottom: 0 }, dwFlags: 0 };
    anyhow::ensure!(unsafe { GetMonitorInfoW(monitor, &mut info) } != 0, "Écran réel indisponible");
    let rx = px.clamp(i64::from(info.rcMonitor.left), i64::from(info.rcMonitor.right - 1));
    let ry = py.clamp(i64::from(info.rcMonitor.top), i64::from(info.rcMonitor.bottom - 1));
    let dx = ((rx - i64::from(sx)) * 65535 / i64::from(sw - 1)) as i32;
    let dy = ((ry - i64::from(sy)) * 65535 / i64::from(sh - 1)) as i32;
    mouse_event(MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK, 0, dx, dy)
}
#[cfg(test)] mod mouse_coordinate_tests {
    use super::*;
    #[test] fn desktop_window_cursor_and_bounds() {
        let screen = (-100, -50, 200, 100);
        let window = Some((-90, -40, 50, 30));
        assert_eq!(mouse_target(0, 0, "window", (0, 0), window, screen).unwrap(), (10 * 65535 / 199, 10 * 65535 / 99));
        assert_eq!(mouse_target(-100, -50, "screen", (0, 0), window, screen).unwrap(), (0, 0));
        assert_eq!(mouse_target(100, 50, "screen", (0, 0), window, screen).unwrap(), (65535, 65535));
        assert_eq!(mouse_target(20, -20, "cursor", (90, 40), window, screen).unwrap(), (65535, 70 * 65535 / 99));
        assert!(mouse_target(50, 0, "window", (0, 0), window, screen).is_err());
        assert!(mouse_target(1, 1, "window", (0, 0), None, screen).is_err());
        assert!(mouse_target(0, 0, "other", (0, 0), window, screen).is_err());
    }
}
// Keep screen 1 stable as the primary monitor. Other screens follow desktop geometry.
fn ordered_monitors(mut screens: Vec<(bool, RECT)>) -> Vec<RECT> {
    screens.sort_by_key(|(primary, r)| (!*primary, r.left, r.top));
    screens.into_iter().map(|(_, r)| r).collect()
}
fn relocate_window(rect: RECT, from: RECT, to: RECT) -> (i32, i32, i32, i32) {
    let width = (rect.right - rect.left).max(1).min((to.right - to.left).max(1));
    let height = (rect.bottom - rect.top).max(1).min((to.bottom - to.top).max(1));
    let x = (to.left + rect.left.saturating_sub(from.left)).clamp(to.left, to.right - width);
    let y = (to.top + rect.top.saturating_sub(from.top)).clamp(to.top, to.bottom - height);
    (x, y, width, height)
}
unsafe extern "system" fn collect_monitor(monitor: windows_sys::Win32::Graphics::Gdi::HMONITOR, _: windows_sys::Win32::Graphics::Gdi::HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    let screens = &mut *(data as *mut Vec<(bool, RECT)>);
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, rcMonitor: std::mem::zeroed(), rcWork: std::mem::zeroed(), dwFlags: 0 };
    if GetMonitorInfoW(monitor, &mut info) != 0 { screens.push((info.dwFlags & 1 != 0, info.rcWork)); }
    1
}
fn monitor_areas() -> Vec<RECT> {
    let mut screens = Vec::new();
    unsafe { EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(collect_monitor), &mut screens as *mut _ as isize); }
    ordered_monitors(screens)
}
struct WindowSearch { query: String, by_title: bool, explicit_self: bool, found: Option<HWND> }
/// Programme sans chemin (« brave.exe ») : Windows ne cherche que dans le PATH avec CreateProcess.
/// Comme l'Exécuter de Windows, on consulte aussi « App Paths » (utilisateur puis machine),
/// où les navigateurs et la plupart des applications installées s'enregistrent.
pub(crate) fn resolve_program(program: &str) -> std::path::PathBuf {
    let plain = !program.contains(['\\', '/', ':']) && program.to_ascii_lowercase().ends_with(".exe");
    if !plain {
        return program.into();
    }
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let key = wide(&format!("Software\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{program}"));
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let mut buffer = [0u16; 1024];
        let mut size = (buffer.len() * 2) as u32;
        let status = unsafe { RegGetValueW(root, key.as_ptr(), std::ptr::null(), RRF_RT_REG_SZ, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut size) };
        if status == 0 {
            let len = buffer.iter().position(|c| *c == 0).unwrap_or(0);
            let path = String::from_utf16_lossy(&buffer[..len]).trim().trim_matches('"').to_owned();
            if !path.is_empty() && std::path::Path::new(&path).is_file() {
                return path.into();
            }
        }
    }
    program.into()
}

unsafe extern "system" fn find_op_window(hwnd: HWND, data: LPARAM) -> BOOL {
    let search = &mut *(data as *mut WindowSearch);
    if IsWindowVisible(hwnd) == 0 || GetWindow(hwnd, GW_OWNER) != std::ptr::null_mut() { return 1; }
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == 0 || pid == std::process::id() && !search.explicit_self { return 1; }
    let matched = if search.by_title {
        let mut text = [0u16; 2048];
        let size = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
        size > 0 && String::from_utf16_lossy(&text[..size as usize]).to_lowercase().contains(&search.query)
    } else {
        use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
        use windows_sys::Win32::Foundation::CloseHandle;
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() { return 1; }
        let mut text = [0u16; 32768];
        let mut size = text.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, text.as_mut_ptr(), &mut size) != 0
            && executable_name(&String::from_utf16_lossy(&text[..size as usize])).as_deref() == Some(search.query.as_str());
        CloseHandle(process);
        ok
    };
    if matched { search.found = Some(hwnd); 0 } else { 1 }
}
/// Fenêtre « réelle » concernée par Fermer/Réduire les autres : visible, sans propriétaire,
/// titrée, pas une fenêtre d'outil, pas le bureau ni la barre des tâches, pas DeckForge.
fn other_window_candidate(class: &str, title: &str, pid: u32, own_pid: u32, visible: bool, owned: bool, tool: bool) -> bool {
    const SHELL: [&str; 6] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "Windows.UI.Core.CoreWindow", "NotifyIconOverflowWindow"];
    visible && !owned && !tool && !title.trim().is_empty() && pid != 0 && pid != own_pid && !SHELL.contains(&class)
}
struct Others { keep: HWND, keep_pid: u32, found: Vec<HWND> }
unsafe extern "system" fn collect_other_window(hwnd: HWND, data: LPARAM) -> BOOL {
    let others = &mut *(data as *mut Others);
    if hwnd == others.keep { return 1; }
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    let mut class = [0u16; 128];
    let class_len = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
    let mut title = [0u16; 256];
    let title_len = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32).max(0) as usize;
    let tool = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0;
    // Les autres fenêtres du même processus que la fenêtre gardée restent intactes (palettes, onglets).
    if pid != others.keep_pid && other_window_candidate(&String::from_utf16_lossy(&class[..class_len]), &String::from_utf16_lossy(&title[..title_len]), pid, std::process::id(), IsWindowVisible(hwnd) != 0, !GetWindow(hwnd, GW_OWNER).is_null(), tool) {
        others.found.push(hwnd);
    }
    1
}
/// Ferme (WM_CLOSE : l'application peut demander d'enregistrer) ou réduit toutes les fenêtres
/// sauf la fenêtre active. Aucun processus n'est tué.
fn other_windows(op: &str, cancel: &AtomicBool) -> Result<String> {
    check(cancel)?;
    let keep = unsafe { GetForegroundWindow() };
    anyhow::ensure!(!keep.is_null(), "Aucune fenêtre active à conserver");
    let mut keep_pid = 0;
    unsafe { GetWindowThreadProcessId(keep, &mut keep_pid); }
    let mut others = Others { keep, keep_pid, found: Vec::new() };
    unsafe { EnumWindows(Some(collect_other_window), &mut others as *mut _ as isize); }
    let mut done = 0;
    for hwnd in others.found.into_iter().take(200) {
        check(cancel)?;
        let ok = unsafe { if op == "close_others" { PostMessageW(hwnd, WM_CLOSE, 0, 0) != 0 } else { ShowWindow(hwnd, SW_SHOWMINNOACTIVE); true } };
        if ok { done += 1; }
    }
    Ok(format!("{} {done} fenêtre(s), fenêtre active conservée", if op == "close_others" { "Fermeture demandée pour" } else { "Réduction de" }))
}
fn window_operation(target: &str, value: &str, op: &str, x: i32, y: i32, w: i32, h: i32, monitor: u32, cancel: &AtomicBool) -> Result<String> {
    check(cancel)?;
    if matches!(op, "close_others" | "minimize_others") {
        anyhow::ensure!(target == "foreground", "Seule la fenêtre active peut être conservée");
        return other_windows(op, cancel);
    }
    let hwnd = if target == "foreground" { unsafe { GetForegroundWindow() } } else {
        let mut search = WindowSearch { query: value.to_lowercase(), by_title: target == "title", explicit_self: target == "title" && value.to_lowercase().contains("deckforge"), found: None };
        unsafe { EnumWindows(Some(find_op_window), &mut search as *mut _ as isize); }
        search.found.unwrap_or(std::ptr::null_mut())
    };
    anyhow::ensure!(!hwnd.is_null() && unsafe { IsWindow(hwnd) } != 0, "Fenêtre introuvable : {target} {value}");
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid); }
    anyhow::ensure!(pid != std::process::id() || target == "title" && value.to_lowercase().contains("deckforge"), "DeckForge ne peut être ciblé que par son titre explicite");
    check(cancel)?;
    unsafe {
        match op {
            "activate" => activate_window(hwnd)?,
            "minimize" => { ShowWindow(hwnd, SW_SHOWMINNOACTIVE); anyhow::ensure!(IsIconic(hwnd) != 0, "Windows a refusé de réduire la fenêtre"); },
            "maximize" => { ShowWindow(hwnd, SW_MAXIMIZE); anyhow::ensure!(IsZoomed(hwnd) != 0, "Windows a refusé de maximiser la fenêtre"); },
            "restore" => { ShowWindow(hwnd, SW_RESTORE); anyhow::ensure!(IsIconic(hwnd) == 0 && IsZoomed(hwnd) == 0, "Windows a refusé de restaurer la fenêtre"); },
            "move" | "resize" | "move_to_monitor" => {
                let mut rect: RECT = std::mem::zeroed();
                anyhow::ensure!(GetWindowRect(hwnd, &mut rect) != 0, "Impossible de lire la position de la fenêtre");
                let screens = if op == "move_to_monitor" { monitor_areas() } else { Vec::new() };
                if op == "move_to_monitor" {
                    anyhow::ensure!(screens.get(monitor as usize - 1).is_some(), "Écran {monitor} introuvable ({} écran(s) connecté(s))", screens.len());
                }
                if IsIconic(hwnd) != 0 || IsZoomed(hwnd) != 0 {
                    ShowWindow(hwnd, SW_RESTORE);
                    anyhow::ensure!(GetWindowRect(hwnd, &mut rect) != 0, "Impossible de lire la position restaurée de la fenêtre");
                }
                let (nx, ny, nw, nh) = if op == "move_to_monitor" {
                    let destination = &screens[monitor as usize - 1];
                    let origin = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, rcMonitor: std::mem::zeroed(), rcWork: std::mem::zeroed(), dwFlags: 0 };
                    anyhow::ensure!(!origin.is_null() && GetMonitorInfoW(origin, &mut info) != 0, "Écran d’origine introuvable");
                    relocate_window(rect, info.rcWork, *destination)
                } else if op == "move" { (x, y, (rect.right - rect.left).max(1), (rect.bottom - rect.top).max(1)) }
                else { (rect.left, rect.top, w, h) };
                anyhow::ensure!(SetWindowPos(hwnd, std::ptr::null_mut(), nx, ny, nw, nh, SWP_NOZORDER | SWP_NOACTIVATE) != 0, "Windows a refusé de déplacer ou redimensionner la fenêtre");
            },
            "toggle_always_on_top" | "always_on_top_on" | "always_on_top_off" => {
                let top = op == "always_on_top_on" || op == "toggle_always_on_top" && (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST == 0);
                anyhow::ensure!(SetWindowPos(hwnd, if top { HWND_TOPMOST } else { HWND_NOTOPMOST }, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) != 0, "Windows a refusé le changement de premier plan permanent");
            },
            _ => bail!("Opération de fenêtre inconnue"),
        }
    }
    Ok(format!("Fenêtre : {}", (Step::WindowOp { target: target.into(), value: value.into(), op: op.into(), x, y, w, h, monitor }).window_op_summary().unwrap_or_default()))
}
#[cfg(test)]
mod window_op_tests {
    use super::*;
    struct ByPid { pid: u32, found: Option<HWND> }
    unsafe extern "system" fn by_pid(hwnd: HWND, data: LPARAM) -> BOOL {
        let s = &mut *(data as *mut ByPid);
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == s.pid && IsWindowVisible(hwnd) != 0 && GetWindow(hwnd, GW_OWNER).is_null() { s.found = Some(hwnd); return 0; }
        1
    }
    /// Banc Windows interactif uniquement : ouvre trois Bloc-notes et réduit les autres fenêtres.
    #[test]
    #[ignore = "banc Windows interactif (DECKFORGE_LAB_WINDOWS=1) : ouvre trois Bloc-notes"]
    fn lab_minimize_others_keeps_only_the_active_window() {
        if std::env::var("DECKFORGE_LAB_WINDOWS").as_deref() != Ok("1") { return; }
        let mut children: Vec<_> = (0..3).map(|_| std::process::Command::new("notepad.exe").spawn().unwrap()).collect();
        std::thread::sleep(Duration::from_secs(3));
        let windows: Vec<HWND> = children.iter().map(|c| {
            let mut s = ByPid { pid: c.id(), found: None };
            unsafe { EnumWindows(Some(by_pid), &mut s as *mut _ as isize); }
            s.found.expect("fenêtre Bloc-notes")
        }).collect();
        let _ = activate_window(windows[2]);
        std::thread::sleep(Duration::from_millis(500));
        let keep = unsafe { GetForegroundWindow() };
        let report = other_windows("minimize_others", &AtomicBool::new(false)).unwrap();
        std::thread::sleep(Duration::from_millis(800));
        let states: Vec<bool> = windows.iter().map(|&h| unsafe { IsIconic(h) != 0 }).collect();
        eprintln!("LAB report={report} foreground_is_notepad3={} iconic={states:?}", keep == windows[2]);
        for c in &mut children { let _ = c.kill(); }
        assert!(unsafe { IsIconic(keep) } == 0, "la fenêtre active ne doit pas être réduite");
        for (i, &h) in windows.iter().enumerate() {
            if h != keep { assert!(states[i], "Bloc-notes {i} aurait dû être réduit"); }
        }
    }
    #[test]
    fn close_others_spares_shell_tools_owned_untitled_and_deckforge() {
        let me = 42;
        assert!(other_window_candidate("Notepad", "Sans titre - Bloc-notes", 7, me, true, false, false));
        for (class, title, pid, visible, owned, tool) in [
            ("Shell_TrayWnd", "", 9, true, false, false),
            ("Progman", "Program Manager", 9, true, false, false),
            ("Notepad", "Bloc-notes", me, true, false, false),
            ("Notepad", "Bloc-notes", 7, false, false, false),
            ("#32770", "Enregistrer sous", 7, true, true, false),
            ("Tool", "Palette", 7, true, false, true),
            ("Chrome_WidgetWin_1", "  ", 7, true, false, false),
        ] {
            assert!(!other_window_candidate(class, title, pid, me, visible, owned, tool), "{class} {title}");
        }
    }
    fn r(left: i32, top: i32, right: i32, bottom: i32) -> RECT { RECT { left, top, right, bottom } }
    #[test]
    fn position_on_another_monitor_preserves_offset_and_clamps() {
        let from = r(0, 0, 1920, 1040);
        let to = r(-1600, 0, 0, 900);
        assert_eq!(relocate_window(r(100, 60, 900, 660), from, to), (-1500, 60, 800, 600));
        assert_eq!(relocate_window(r(1800, 900, 2600, 1500), from, to), (-800, 300, 800, 600));
        let sorted = ordered_monitors(vec![(false, to), (true, from)]);
        assert_eq!(sorted[0].left, 0);
        assert_eq!(sorted[1].left, -1600);
    }
}
fn close_foreground_window() -> Result<()> {
    let window = unsafe { GetForegroundWindow() };
    anyhow::ensure!(!window.is_null(), "Aucune fenêtre active à fermer");
    let mut process_id = 0u32;
    unsafe { GetWindowThreadProcessId(window, &mut process_id) };
    anyhow::ensure!(process_id != std::process::id(), "DeckForge ne peut pas se fermer lui-même");
    let posted = unsafe { PostMessageW(window, WM_CLOSE, 0, 0) };
    anyhow::ensure!(posted != 0, "Windows a refusé de fermer la fenêtre");
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
// Search only visible top-level windows with an accessible executable identity.
// A process without a visible window is not activated and follows the normal launch path.
struct ExecutableSearch { name: String, found: Option<HWND> }
fn executable_name(program: &str) -> Option<String> {
    std::path::Path::new(program).file_name().map(|s| s.to_string_lossy().to_lowercase())
}
unsafe extern "system" fn enum_executable_window(hwnd: HWND, data: LPARAM) -> BOOL {
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows_sys::Win32::Foundation::CloseHandle;
    let search = &mut *(data as *mut ExecutableSearch);
    if IsWindowVisible(hwnd) == 0 || GetWindow(hwnd, GW_OWNER) != std::ptr::null_mut() { return 1; }
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == 0 || pid == std::process::id() { return 1; }
    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if process.is_null() { return 1; }
    let mut buffer = [0u16; 32768];
    let mut len = buffer.len() as u32;
    let matches = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut len) != 0
        && executable_name(&String::from_utf16_lossy(&buffer[..len as usize])).as_deref() == Some(search.name.as_str());
    CloseHandle(process);
    if matches { search.found = Some(hwnd); 0 } else { 1 }
}
fn find_window_by_exe(program: &str) -> Option<HWND> {
    let mut search = ExecutableSearch { name: executable_name(program)?, found: None };
    unsafe { EnumWindows(Some(enum_executable_window), &mut search as *mut _ as isize); }
    search.found
}
fn activate_window(hwnd: HWND) -> Result<()> {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    unsafe {
        if IsIconic(hwnd) != 0 { ShowWindow(hwnd, SW_RESTORE); }
        if GetForegroundWindow() == hwnd { return Ok(()); }
        let active = || GetForegroundWindow() == hwnd;
        let mut ok = SetForegroundWindow(hwnd) != 0 && active();
        if !ok {
            // Touche D6 : DeckForge n'est pas au premier plan et Windows peut refuser
            // l'activation. Partager l'entrée avec la fenêtre active est le moyen documenté.
            let foreground = GetForegroundWindow();
            let theirs = if foreground.is_null() { 0 } else { GetWindowThreadProcessId(foreground, std::ptr::null_mut()) };
            let ours = GetCurrentThreadId();
            if theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, 1) != 0 {
                BringWindowToTop(hwnd);
                SetForegroundWindow(hwnd);
                AttachThreadInput(ours, theirs, 0);
            }
            ok = active();
        }
        // Windows foreground lock policy may still reject activation; never claim success in that case.
        anyhow::ensure!(ok, "Windows a refusé d’activer l’application déjà ouverte (restriction du premier plan)");
    }
    // Laisse la fenêtre prendre le focus clavier avant un raccourci (Ctrl+T, etc.).
    std::thread::sleep(Duration::from_millis(120));
    Ok(())
}
pub(crate) fn find_window(query: &str) -> Option<HWND> {
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

    #[test]
    fn executable_identity_is_pure() {
        assert_eq!(super::executable_name("C:\\Apps\\NOTEPAD.EXE").as_deref(), Some("notepad.exe"));
        assert_eq!(super::executable_name("notepad.exe").as_deref(), Some("notepad.exe"));
    }
    #[test]
    fn toggle_alternates_and_badge_follows() {
        let key = crate::model::Key { steps: vec![crate::model::Step::Toggle { a: vec![], b: vec![] }], ..crate::model::Key::default() };
        let id = super::identity("t", "p", 7);
        super::ACTIVE_KEY.with(|k| *k.borrow_mut() = id.clone());
        assert_eq!(super::toggle_badge(&id, &key).as_deref(), Some("1/2"));
        assert_eq!(super::next_branch(&[0]), 0);
        assert_eq!(super::toggle_badge(&id, &key).as_deref(), Some("2/2"));
        assert_eq!(super::next_branch(&[0]), 1);
        assert_eq!(super::toggle_badge(&id, &key).as_deref(), Some("1/2"));
        let json = serde_json::to_string(&key.steps[0]).unwrap();
        assert!(json.contains("toggle"), "{json}");
    }
    use super::*;
    #[test]
    fn sound_simulation_does_not_open_missing_files() {
        let (tx, rx) = std::sync::mpsc::channel();
        let key = Key { steps: vec![Step::PlaySound { path: "Z:\\missing.wav".into(), volume: 40 }, Step::StopSounds], ..Key::default() };
        run(&key, true, &AtomicBool::new(false), &tx, &crate::model::ObsSettings::default(), None, &std::sync::Mutex::new(KeyRegistry::default())).unwrap();
        assert!(rx.try_iter().any(|event| matches!(event, Event::Log(s) if s.contains("Jouer un son"))));
    }
    #[test]
    fn disabled_nested_blocks_are_skipped_during_execution_and_simulation() {
        for dry in [false, true] {
            let (tx, rx) = std::sync::mpsc::channel();
            let mut vars = std::collections::BTreeMap::new();
            let mut budget = 10;
            let steps = vec![Step::Repeat { count: 1, steps: vec![
                Step::Disabled { step: Box::new(Step::AutoHotkey { executable: "missing.exe".into(), script: "fail".into(), args: vec![] }) },
                Step::Disabled { step: Box::new(Step::SetVariable { name: "bad".into(), value: "yes".into(), persistent: false, }) },
                Step::SetVariable { name: "good".into(), value: "yes".into(), persistent: false, },
            ] }];
            run_steps(&steps, dry, &AtomicBool::new(false), &tx, &mut vars, &mut budget, &mut None, &[], &crate::model::ObsSettings::default(), None, &std::sync::Mutex::new(KeyRegistry::default())).unwrap();
            assert!(!vars.contains_key("bad"));
            if !dry { assert_eq!(vars.get("good").map(String::as_str), Some("yes")); }
            assert_eq!(budget, 8);
            assert!(!rx.try_iter().any(|event| matches!(event, Event::Log(s) if s.contains("AutoHotkey"))));
        }
    }

    #[test]
    fn conditional_blocks_repeat_and_variables_choose_only_the_matching_branch() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut variables = std::collections::BTreeMap::new();
        let mut budget = 100;
        let steps = vec![
            Step::SetVariable {
                name: "mode".into(),
                value: "travail".into(),
                    persistent: false,
                },
            Step::Repeat {
                count: 3,
                steps: vec![Step::IfVariable {
                    name: "mode".into(),
                    value: "travail".into(),
                    then_steps: vec![Step::SetVariable {
                        name: "result".into(),
                        value: "oui".into(),
                   persistent: false, }],
                    else_steps: vec![Step::AutoHotkey {
                        executable: "missing.exe".into(),
                        script: "This branch must not execute".into(),
                        args: vec![],
                    }],
                }],
            },
        ];
        run_steps(
            &steps,
            false,
            &AtomicBool::new(false),
            &tx,
            &mut variables,
            &mut budget,
            &mut None,
            &[],
            &crate::model::ObsSettings::default(),
            None,
            &std::sync::Mutex::new(KeyRegistry::default()),
        )
        .unwrap();
        assert_eq!(variables["result"], "oui");
        assert_eq!(budget, 92);
        assert_eq!(
            rx.try_iter()
                .filter(|e| matches!(e,Event::Log(s) if s.contains("Script AutoHotkey")))
                .count(),
            0
        );
    }
    #[test]
    fn dry_nested_scripts_and_audio_have_no_side_effects() {
        let (tx, _) = std::sync::mpsc::channel();
        let key = Key {
            steps: vec![Step::IfVariable {
                name: "x".into(),
                value: "y".into(),
                then_steps: vec![crate::automation::step(13)],
                else_steps: vec![crate::automation::step(18)],
            }],
            ..Key::default()
        };
        run(&key, true, &AtomicBool::new(false), &tx, &crate::model::ObsSettings::default(), None, &std::sync::Mutex::new(KeyRegistry::default())).unwrap();
    }
    #[test]
    fn nested_execution_budget_stops_before_another_block_runs() {
        let (tx, _) = std::sync::mpsc::channel();
        let mut variables = std::collections::BTreeMap::new();
        let mut budget = 1;
        let err = run_steps(
            &[
                Step::SetVariable {
                    name: "a".into(),
                    value: "1".into(),
               persistent: false, },
                Step::SetVariable {
                    name: "b".into(),
                    value: "2".into(),
               persistent: false, },
            ],
            false,
            &AtomicBool::new(false),
            &tx,
            &mut variables,
            &mut budget,
            &mut None,
            &[],
            &crate::model::ObsSettings::default(),
            None,
            &std::sync::Mutex::new(KeyRegistry::default()),
        )
        .unwrap_err();
        assert!(err.to_string().contains("10 000"));
        assert!(!variables.contains_key("b"));
    }
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
        run(&k, true, &AtomicBool::new(false), &tx, &crate::model::ObsSettings::default(), None, &std::sync::Mutex::new(KeyRegistry::default())).unwrap();
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
        run(&k, false, &AtomicBool::new(false), &tx, &crate::model::ObsSettings::default(), None, &std::sync::Mutex::new(KeyRegistry::default())).unwrap();
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

#[cfg(test)]
mod launch_tests {
    #[test]
    fn programs_without_path_are_found_in_app_paths() {
        assert_eq!(super::resolve_program("C:\\Outils\\x.exe"), std::path::PathBuf::from("C:\\Outils\\x.exe"));
        assert_eq!(super::resolve_program("introuvable-deckforge.exe"), std::path::PathBuf::from("introuvable-deckforge.exe"));
        // WordPad s'enregistre dans App Paths sur Windows 10 et 11 (sans être dans le PATH).
        let wordpad = super::resolve_program("wordpad.exe");
        if wordpad.is_absolute() {
            assert!(wordpad.is_file(), "{wordpad:?}");
        }
        eprintln!("APP_PATHS wordpad.exe -> {wordpad:?}");
    }
}

#[cfg(test)]
mod clipboard_pure_tests {
    use super::*;
    #[test]
    fn clipboard_template_resolves_variables_without_windows_access() {
        let vars = std::collections::BTreeMap::from([("texte_colle".into(), "é🙂".into())]);
        assert_eq!(interpolate("{texte_colle}", &vars), "é🙂");
        assert_eq!(interpolate("{{texte_colle}", &vars), "{texte_colle}");
    }
    #[test]
    fn dry_clipboard_only_logs_and_never_reads_or_writes() {
        let (tx, rx) = std::sync::mpsc::channel();
        let key = Key { steps: vec![Step::Clipboard { op: "read_to_variable".into(), value: String::new(), name: "x".into() }, Step::Clipboard { op: "restore".into(), value: String::new(), name: "x".into() }], ..Key::default() };
        run(&key, true, &AtomicBool::new(false), &tx, &crate::model::ObsSettings::default(), None, &std::sync::Mutex::new(KeyRegistry::default())).unwrap();
        assert_eq!(rx.try_iter().filter(|event| matches!(event, Event::Log(s) if s.contains("non consulté ni modifié"))).count(), 2);
    }
}
