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
use windows_sys::Win32::{
    Foundation::{BOOL, HWND, LPARAM},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

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
pub struct Engine {
    pub running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    tx: Sender<Event>,
    last_finished: Arc<std::sync::Mutex<Instant>>,
}
impl Engine {
    pub fn new(tx: Sender<Event>) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(AtomicBool::new(false)),
            tx,
            last_finished: Arc::new(std::sync::Mutex::new(
                Instant::now() - Duration::from_secs(1),
            )),
        }
    }
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
    pub fn allows_global_press(&self, at: Instant) -> bool {
        !self.running.load(Ordering::SeqCst)
            && at
                > *self.last_finished.lock().unwrap_or_else(|e| e.into_inner())
                    + Duration::from_millis(150)
    }
    pub fn start(&self, key: Key, dry: bool, initial_delay: u64, obs: crate::model::ObsSettings) {
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
        let last_finished = self.last_finished.clone();
        // Physical press: capture before spawning; delayed test: capture after countdown.
        let captured = if !dry && initial_delay == 0 && has_app_volume(&key.steps) {
            Some(crate::app_audio::capture("foreground", true))
        } else { None };
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if !dry {
                    wait(initial_delay, &cancel)?;
                }
                ACTIVE_KEY.with(|id| *id.borrow_mut() = key.runtime_id.clone().unwrap_or_else(|| serde_json::to_string(&key).unwrap_or_default()));
                let foreground = if dry || !has_app_volume(&key.steps) { None }
                    else if let Some(captured) = captured { Some(captured?) }
                    else { Some(crate::app_audio::capture("foreground", true)?) };
                run(&key, dry, &cancel, &tx, &obs, foreground.as_ref())
            }));
            let message = match result {
                Ok(Ok(())) => "Macro terminée".into(),
                Ok(Err(e)) => format!("Macro arrêtée : {e}"),
                Err(_) => "Erreur interne : macro interrompue".into(),
            };
            let _ = tx.send(Event::Log(message));
            *last_finished.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
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
fn run(key: &Key, dry: bool, cancel: &AtomicBool, tx: &Sender<Event>, obs: &crate::model::ObsSettings, foreground: Option<&crate::app_audio::Target>) -> Result<()> {
    let _ = tx.send(Event::Log(format!(
        "{} — {} ({} répétition(s))",
        if dry { "SIMULATION" } else { "EXÉCUTION" },
        key.label,
        key.repeat
    )));
    let mut variables = std::collections::BTreeMap::new();
    let mut budget = 10_000usize;
    for _ in 0..if dry { 1 } else { key.repeat } {
        run_steps(
            &key.steps,
            dry,
            cancel,
            tx,
            &mut variables,
            &mut budget,
            &[],
            obs,
            foreground,
        )?;
    }
    let _ = tx.send(Event::Block(vec![]));
    Ok(())
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
    parent: &[usize],
    obs: &crate::model::ObsSettings,
    foreground: Option<&crate::app_audio::Target>,
) -> Result<()> {
    for (idx, step) in steps.iter().enumerate() {
        if step.is_disabled() { continue; }
        check(cancel)?;
        anyhow::ensure!(*budget > 0, "Limite de 10 000 blocs exécutés atteinte");
        *budget -= 1;
        let mut path = parent.to_vec();
        path.push(idx);
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
                Step::TextOp { .. } => {
                    let _ = tx.send(Event::Log(format!("Simulation : {} (aucune variable modifiée)", step.text_op_summary().unwrap_or_default())));
                }
                Step::Math { name, op, value, min, max } => {
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
                        run_steps(branch, true, cancel, tx, variables, budget, &[path.clone(), vec![branch_id]].concat(), obs, foreground)?;
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
                        &[path.clone(), vec![0]].concat(),
                        obs,
                        foreground,
                    )?;
                    run_steps(
                        else_steps,
                        true,
                        cancel,
                        tx,
                        variables,
                        budget,
                        &[path.clone(), vec![1]].concat(),
                        obs,
                        foreground,
                    )?;
                }
                Step::Toggle { a, b } => {
                    for (branch, list) in [a, b].into_iter().enumerate() {
                        run_steps(list, true, cancel, tx, variables, budget, &[path.clone(), vec![branch]].concat(), obs, foreground)?;
                    }
                },
                Step::Repeat { steps, .. } => run_steps(
                    steps,
                    true,
                    cancel,
                    tx,
                    variables,
                    budget,
                    &[path.clone(), vec![0]].concat(),
                    obs,
                    foreground,
                )?,
                _ => {}
            }
            continue;
        }
        match step {
            Step::Disabled { .. } => {},
            Step::Toggle { a, b } => {
                let branch = next_branch(&path);
                let _ = tx.send(Event::ToggleVisual);
                run_steps(if branch == 0 { a } else { b }, dry, cancel, tx, variables, budget, &[path.clone(), vec![branch]].concat(), obs, foreground)?;
            },
            Step::PlaySound { path, volume } => crate::soundboard::play(path, *volume)?,
            Step::StopSounds => crate::soundboard::stop(),
            Step::UdpSend { host, port, text } => crate::network_actions::udp_send(host, *port, text)?,
            Step::HttpRequest { method, url, body, content_type } => { let reply = crate::network_actions::http_request(method, url, body, content_type)?; let _ = tx.send(Event::Log(reply)); },
            Step::D6Brightness { mode, value } => { let _ = tx.send(Event::D6Brightness(mode.clone(), *value)); },
            Step::D6Sleep => { let _ = tx.send(Event::D6Sleep); },
            Step::CloseWindow => close_foreground_window()?,
            Step::MouseClick { button, double } => mouse_click(button, *double)?,
            Step::MouseMove { x, y, relative } => mouse_move(*x, *y, *relative)?,
            Step::MouseScroll { amount } => mouse_event(MOUSEEVENTF_WHEEL, amount.wrapping_mul(120) as u32, 0, 0)?,
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
            Step::TextOp { name, op, value, find } => {
                let current = variables.get(name).map(String::as_str).unwrap_or("");
                let value = interpolate(value, variables);
                let find = interpolate(find, variables);
                let result = text_operation(current, op, &value, &find)?;
                variables.insert(name.clone(), result.clone());
                let _ = tx.send(Event::Log(format!("Texte : {name} = {} caractères", result.chars().count())));
            }
            Step::Math { name, op, value, min, max } => {
                let current = variables.get(name).map(|s| s.parse::<f64>()).transpose()
                    .map_err(|_| anyhow::anyhow!("Calcul : variable « {name} » non numérique"))?.unwrap_or(0.0);
                let operand = if op == "random" { 0.0 } else { interpolate(value, variables).trim().parse::<f64>()
                    .map_err(|_| anyhow::anyhow!("Calcul : valeur numérique invalide pour « {name} »"))? };
                let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| anyhow::anyhow!("Horloge système indisponible"))?.subsec_nanos();
                let result = calculate(current, op, operand, *min, *max, f64::from(nanos) / 1_000_000_000.0)?;
                let value = result.to_string();
                variables.insert(name.clone(), value.clone());
                let _ = tx.send(Event::Log(format!("Calcul : {name} = {value}")));
            }
            Step::SetVariable { name, value } => {
                variables.insert(name.clone(), value.clone());
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
                run_steps(if result { then_steps } else { else_steps }, false, cancel, tx, variables, budget, &[path.clone(), vec![branch_id]].concat(), obs, foreground)?;
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
                    &[path.clone(), vec![branch_id]].concat(),
                    obs,
                    foreground,
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
                    &[path.clone(), vec![branch_id]].concat(),
                    obs,
                    foreground,
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
                        &[path.clone(), vec![0]].concat(),
                        obs,
                        foreground,
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
            Step::Text { text } => {
                for unit in text.encode_utf16() {
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
fn mouse_move(x: i32, y: i32, relative: bool) -> Result<()> {
    if relative { mouse_event(MOUSEEVENTF_MOVE, 0, x, y) }
    else {
        let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        anyhow::ensure!(width > 1 && height > 1, "Écran introuvable");
        let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
        mouse_event(MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK, 0,
            ((i64::from(x - left) * 65535) / i64::from(width - 1)).clamp(0, 65535) as i32,
            ((i64::from(y - top) * 65535) / i64::from(height - 1)).clamp(0, 65535) as i32)
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
    unsafe {
        if IsIconic(hwnd) != 0 { ShowWindow(hwnd, SW_RESTORE); }
        // Windows foreground lock policy may reject activation; never claim success in that case.
        anyhow::ensure!(SetForegroundWindow(hwnd) != 0 && GetForegroundWindow() == hwnd,
            "Windows a refusé d’activer l’application déjà ouverte (restriction du premier plan)");
    }
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
        run(&key, true, &AtomicBool::new(false), &tx, &crate::model::ObsSettings::default(), None).unwrap();
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
                Step::Disabled { step: Box::new(Step::SetVariable { name: "bad".into(), value: "yes".into() }) },
                Step::SetVariable { name: "good".into(), value: "yes".into() },
            ] }];
            run_steps(&steps, dry, &AtomicBool::new(false), &tx, &mut vars, &mut budget, &[], &crate::model::ObsSettings::default(), None).unwrap();
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
            },
            Step::Repeat {
                count: 3,
                steps: vec![Step::IfVariable {
                    name: "mode".into(),
                    value: "travail".into(),
                    then_steps: vec![Step::SetVariable {
                        name: "result".into(),
                        value: "oui".into(),
                    }],
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
            &[],
            &crate::model::ObsSettings::default(),
            None,
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
        run(&key, true, &AtomicBool::new(false), &tx, &crate::model::ObsSettings::default(), None).unwrap();
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
                },
                Step::SetVariable {
                    name: "b".into(),
                    value: "2".into(),
                },
            ],
            false,
            &AtomicBool::new(false),
            &tx,
            &mut variables,
            &mut budget,
            &[],
            &crate::model::ObsSettings::default(),
            None,
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
        run(&k, true, &AtomicBool::new(false), &tx, &crate::model::ObsSettings::default(), None).unwrap();
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
        run(&k, false, &AtomicBool::new(false), &tx, &crate::model::ObsSettings::default(), None).unwrap();
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
