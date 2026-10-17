use crate::model::{Key, Step};
use anyhow::{bail, Result};
pub const NAMES: &[&str] = &[
    "Raccourci clavier",
    "Saisir du texte",
    "Application",
    "Site web",
    "Média / volume",
    "Attendre",
    "Attendre une fenêtre",
    "Changer de profil",
    "Page suivante",
    "Page précédente",
    "Retour au dossier",
    "Page 1",
    "Créer un dossier",
    "Basculer l’audio",
    "Si une fenêtre est ouverte",
    "Répéter des blocs",
    "Définir une variable",
    "Si une variable correspond",
    "Script AutoHotkey",
    "OBS : scène", "OBS : enregistrement", "OBS : diffusion", "OBS : caméra",
    "OBS : replay", "OBS : micro", "OBS : source", "Fermer la fenêtre active", "Clic souris", "Déplacer la souris", "Défilement souris", "Jouer un son", "Arrêter les sons", "Luminosité du D6", "Veille du D6", "Envoyer en UDP", "Requête HTTP", "Touche à deux états", "Volume d’une application",
];
pub fn step(kind: usize) -> Step {
    match kind {
        37 => Step::AppVolume { target: "foreground".into(), action: "up".into(), step: 5, level: 50, include_children: true },
        13 => Step::AudioSwitch {
            first: String::new(),
            second: String::new(),
            input: false,
            first_label: "Casque".into(),
            second_label: "Enceintes".into(),
            first_icon: String::new(),
            second_icon: String::new(),
        },
        14 => Step::IfWindow {
            title: "Mon application".into(),
            then_steps: vec![Step::Delay { ms: 250 }],
            else_steps: vec![],
        },
        15 => Step::Repeat {
            count: 2,
            steps: vec![Step::Delay { ms: 250 }],
        },
        16 => Step::SetVariable {
            name: "mode".into(),
            value: "travail".into(),
        },
        17 => Step::IfVariable {
            name: "mode".into(),
            value: "travail".into(),
            then_steps: vec![Step::Delay { ms: 250 }],
            else_steps: vec![],
        },
        30 => Step::PlaySound { path: "choisir.wav".into(), volume: 100 },
        31 => Step::StopSounds,
        32 => Step::D6Brightness { mode: "set".into(), value: 70 },
        33 => Step::D6Sleep,
        34 => Step::UdpSend { host: "127.0.0.1".into(), port: 8099, text: "message".into() },
        36 => Step::Toggle { a: vec![Step::Delay { ms: 100 }], b: vec![Step::Delay { ms: 200 }] },
        35 => Step::HttpRequest { method: "GET".into(), url: "http://127.0.0.1:8088/api".into(), body: String::new(), content_type: "text/plain".into() },
        26 => Step::CloseWindow,
        27 => Step::MouseClick { button: "left".into(), double: false },
        28 => Step::MouseMove { x: 0, y: 0, relative: true },
        29 => Step::MouseScroll { amount: 1 },
        19 => Step::ObsScene { name: "Scène".into() },
        20 => Step::ObsRecord { mode: "toggle".into() },
        21 => Step::ObsStream { mode: "toggle".into() },
        22 => Step::ObsVirtualCam { mode: "toggle".into() },
        23 => Step::ObsReplay { mode: "save".into() },
        24 => Step::ObsMute { input: "Micro/Aux".into(), mode: "toggle".into() },
        25 => Step::ObsSourceVisibility { scene: "Scène".into(), source: "Source".into(), mode: "toggle".into() },
        _ => Step::AutoHotkey {
            executable: detect_ahk(),
            script: "#Requires AutoHotkey v2.0\n; Écris ton script ici.\n".into(),
            args: vec![],
        },
    }
}
pub fn detect_ahk() -> String {
    for base in [
        std::env::var_os("ProgramFiles"),
        std::env::var_os("LOCALAPPDATA"),
    ]
    .into_iter()
    .flatten()
    {
        for rel in [
            "AutoHotkey/v2/AutoHotkey64.exe",
            "Programs/AutoHotkey/v2/AutoHotkey64.exe",
        ] {
            let p = std::path::PathBuf::from(&base).join(rel);
            if p.is_file() {
                return p.to_string_lossy().into_owned();
            }
        }
    }
    String::new()
}
fn literal(text: &str) -> String {
    format!(
        "\"{}\"",
        text.replace('`', "``")
            .replace('"', "`\"")
            .replace('\r', "`r")
            .replace('\n', "`n")
    )
}
/// Export is deliberately atomic: unsupported blocks cause an error, never omission.
pub fn export(key: &Key) -> Result<String> {
    fn steps(out: &mut String, list: &[Step], indent: usize) -> Result<()> {
        let pad = "    ".repeat(indent);
        for s in list {
            if s.is_disabled() { continue; }
            let line = match s {
                Step::Delay { ms } => format!("Sleep({ms})"),
                Step::Text { text } => format!("SendText({})", literal(text)),
                Step::Url { url } => format!("Run({})", literal(url)),
                Step::Hotkey { keys } => {
                    let codes = crate::model::parse_hotkey(keys)?;
                    let mut prefix = String::new();
                    let mut body = String::new();
                    for vk in codes {
                        match vk {
                            0x11 => prefix.push('^'),
                            0x10 => prefix.push('+'),
                            0x12 => prefix.push('!'),
                            0x5b => prefix.push('#'),
                            _ => body.push_str(&format!("{{vk{vk:02X}}}")),
                        }
                    }
                    format!("Send({})", literal(&format!("{prefix}{body}")))
                }
                Step::Media { command } => format!(
                    "Send({})",
                    literal(&format!(
                        "{{vk{:02X}}}",
                        crate::model::media_vk(command)
                            .ok_or_else(|| anyhow::anyhow!("Commande média inconnue"))?
                    ))
                ),
                Step::SetVariable { name, value } => {
                    format!("vars[{}] := {}", literal(name), literal(value))
                }
                Step::Repeat {
                    count,
                    steps: inner,
                } => {
                    out.push_str(&format!("{pad}Loop {count} {{\n"));
                    steps(out, inner, indent + 1)?;
                    out.push_str(&format!("{pad}}}\n"));
                    continue;
                }
                Step::IfWindow {
                    title,
                    then_steps,
                    else_steps,
                }
                | Step::IfVariable {
                    name: title,
                    then_steps,
                    else_steps,
                    ..
                } => {
                    let condition = if let Step::IfVariable { name, value, .. } = s {
                        format!("vars.Get({}, \"\") = {}", literal(name), literal(value))
                    } else {
                        format!(
                            "WinExist({})",
                            literal(&format!("i){}", regex_escape(title)))
                        )
                    };
                    out.push_str(&format!("{pad}if {condition} {{\n"));
                    steps(out, then_steps, indent + 1)?;
                    out.push_str(&format!("{pad}}} else {{\n"));
                    steps(out, else_steps, indent + 1)?;
                    out.push_str(&format!("{pad}}}\n"));
                    continue;
                }
                _ => bail!(
                    "Export AutoHotkey indisponible pour le bloc « {} ». Aucun bloc n’a été omis.",
                    s.name()
                ),
            };
            out.push_str(&format!("{pad}{line}\n"));
        }
        Ok(())
    }
    let mut out="#Requires AutoHotkey v2.0\n#SingleInstance Off\nSetTitleMatchMode(\"RegEx\")\nvars := Map()\n".to_string();
    out.push_str(&format!("Loop {} {{\n", key.repeat));
    steps(&mut out, &key.steps, 1)?;
    out.push_str("}\n");
    Ok(out)
}
fn regex_escape(s: &str) -> String {
    let mut r = String::new();
    for c in s.chars() {
        if "\\.^$|?*+()[]{}".contains(c) {
            r.push('\\');
        }
        r.push(c);
    }
    r
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_unsupported_block_is_omitted_from_export() {
        let key = Key { steps: vec![Step::Disabled { step: Box::new(Step::AudioSwitch {
            first: "x".into(), second: "y".into(), input: false,
            first_label: String::new(), second_label: String::new(), first_icon: String::new(), second_icon: String::new()
        }) }, Step::Delay { ms: 42 }], ..Key::default() };
        let output = export(&key).unwrap();
        assert!(output.contains("Sleep(42)"));
        assert!(!output.contains("audio"));
    }

    #[test]
    fn installed_ahk_runs_a_local_script_and_cancellation_stops_it() {
        let Some(executable) = std::env::var_os("DECKFORGE_TEST_AHK") else {
            return;
        };
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("marker.txt");
        let cancel = std::sync::atomic::AtomicBool::new(false);
        run_script(
            &executable.to_string_lossy(),
            "#Requires AutoHotkey v2.0\nFileAppend(\"deckforge-ok\", A_Args[1])\n",
            &[target.to_string_lossy().into_owned()],
            &cancel,
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "deckforge-ok");
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let c = cancel.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(150));
            c.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        let start = std::time::Instant::now();
        assert!(run_script(
            &executable.to_string_lossy(),
            "#Requires AutoHotkey v2.0\nSleep(60000)\n",
            &[],
            &cancel
        )
        .is_err());
        thread.join().unwrap();
        assert!(start.elapsed() < std::time::Duration::from_secs(3));
    }
    #[test]
    fn ahk_export_preserves_quotes_newlines_conditions_and_repeat() {
        let key = Key {
            steps: vec![
                Step::Text {
                    text: "a\"b`\nc".into(),
                },
                Step::Repeat {
                    count: 3,
                    steps: vec![Step::Hotkey {
                        keys: "CTRL+SHIFT+S".into(),
                    }],
                },
                Step::IfWindow {
                    title: "Chrome (test)".into(),
                    then_steps: vec![Step::Delay { ms: 25 }],
                    else_steps: vec![],
                },
            ],
            ..Key::default()
        };
        let text = export(&key).unwrap();
        assert!(text.contains("a`\"b```nc"));
        assert!(text.contains("Loop 3"));
        assert!(text.contains("^+{vk53}"));
        assert!(text.contains("Chrome \\(test\\)"));
        assert!(export(&Key {
            steps: vec![crate::automation::step(13)],
            ..Key::default()
        })
        .is_err());
    }
}
pub fn run_script(
    executable: &str,
    script: &str,
    args: &[String],
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    use std::{
        io::{Read, Seek, SeekFrom, Write},
        os::windows::process::CommandExt,
        process::{Command, Stdio},
    };
    let mut file = tempfile::Builder::new().suffix(".ahk").tempfile()?;
    file.write_all(b"\xEF\xBB\xBF")?;
    file.write_all(script.as_bytes())?;
    file.flush()?;
    let mut errors = tempfile::tempfile()?;
    let child = Command::new(executable)
        .arg("/ErrorStdOut=UTF-8")
        .arg(file.path())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(errors.try_clone()?))
        .creation_flags(0x08000000)
        .spawn()?;
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Child(child);
    loop {
        if crate::engine::check(cancel).is_err() {
            bail!("Script AutoHotkey annulé");
        }
        if let Some(status) = child.0.try_wait()? {
            if !status.success() {
                let mut msg = String::new();
                errors.seek(SeekFrom::Start(0))?;
                errors.take(4096).read_to_string(&mut msg)?;
                bail!("AutoHotkey : {} ({status})", msg.trim());
            }
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    Ok(())
}

#[cfg(test)]
mod native_close_tests {
    #[test]
    fn export_close_window_fails_atomically() {
        let key = crate::model::Key { steps: vec![crate::model::Step::CloseWindow], ..Default::default() };
        assert!(super::export(&key).is_err());
    }
}
