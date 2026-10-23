//! Vérification et installation des versions publiées de NovyDeck (Windows uniquement).
#![cfg(windows)]
use std::{cmp::Ordering, fs, path::{Path, PathBuf}, process::Command};
use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const LATEST_URL: &str = "https://api.github.com/repos/HatiFlash/NovyDeck/releases/latest";
const MAX_SETUP: usize = 200 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Latest {
    pub version: String,
    pub page: String,
    pub setup_url: String,
    pub setup_size: u64,
    pub sha_url: String,
    pub notes: String,
}
#[derive(Clone, Debug)]
pub enum Status { Current, New(Latest), Ahead }

#[derive(Deserialize)]
struct Release { tag_name: String, html_url: String, draft: bool, prerelease: bool, #[serde(default)] body: Option<String>, assets: Vec<Asset> }
#[derive(Deserialize)]
struct Asset { name: String, browser_download_url: String, size: u64 }

/// Turn bilingual GitHub Markdown into bounded, plain-text release notes.
pub fn release_notes(markdown: &str, lang: &str) -> String {
    let wanted = if lang == "fr" { "Français" } else { "English" };
    let lines: Vec<&str> = markdown.lines().collect();
    let section = lines.iter().position(|line| line.trim() == format!("## {wanted}"));
    let selected: Vec<&str> = if let Some(start) = section {
        lines[start + 1..].iter().copied().take_while(|line| !line.trim_start().starts_with("## ")).collect()
    } else if lines.iter().any(|line| matches!(line.trim(), "## English" | "## Français")) {
        // A bilingual document with only the other language: show what is available.
        let start = lines.iter().position(|line| matches!(line.trim(), "## English" | "## Français")).unwrap();
        lines[start + 1..].iter().copied().take_while(|line| !line.trim_start().starts_with("## ")).collect()
    } else { lines };
    let mut clean = Vec::new();
    for line in selected {
        let line = line.trim();
        if line.starts_with("English · [Français](") || line.starts_with("Français · [English](") { continue; }
        let line = line.trim_start_matches('#').trim_start();
        let mut out = String::new();
        let mut rest = line;
        while let Some(open) = rest.find('[') {
            out.push_str(&rest[..open]);
            if let Some(close) = rest[open + 1..].find("](") {
                let title_end = open + 1 + close;
                if let Some(end) = rest[title_end + 2..].find(')') {
                    out.push_str(&rest[open + 1..title_end]);
                    rest = &rest[title_end + 3 + end..];
                    continue;
                }
            }
            out.push('[');
            rest = &rest[open + 1..];
        }
        out.push_str(rest);
        let out = out.replace("**", "").replace('`', "");
        let out = out.trim();
        if out.is_empty() {
            if clean.last().is_some_and(|previous: &String| !previous.is_empty()) { clean.push(String::new()); }
        } else { clean.push(out.to_owned()); }
    }
    let text = clean.join("\n");
    let text = text.trim();
    if text.chars().count() <= 4000 { return text.to_owned(); }
    let mut cut: String = text.chars().take(3999).collect();
    if let Some(space) = cut.rfind(char::is_whitespace) {
        if space > 3800 { cut.truncate(space); }
    }
    format!("{}…", cut.trim_end())
}

fn version(s: &str) -> Result<[u64; 3]> {
    let s = s.strip_prefix('v').unwrap_or(s);
    let parts: Vec<_> = s.split('.').collect();
    anyhow::ensure!(parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())), "Version invalide : {s}");
    Ok([parts[0].parse()?, parts[1].parse()?, parts[2].parse()?])
}
pub fn compare(local: &str, remote: &str) -> Result<Ordering> { Ok(version(local)?.cmp(&version(remote)?)) }

fn https(raw: &str) -> Result<()> {
    let url = url::Url::parse(raw).context("Adresse de mise à jour invalide")?;
    anyhow::ensure!(url.scheme() == "https" && url.host_str().is_some() && url.username().is_empty() && url.password().is_none(), "Adresse HTTPS de mise à jour invalide");
    Ok(())
}
pub fn parse_latest(bytes: &[u8]) -> Result<Latest> {
    let release: Release = serde_json::from_slice(bytes).context("Réponse GitHub invalide")?;
    anyhow::ensure!(!release.draft && !release.prerelease, "Version publiée non définitive");
    version(&release.tag_name)?;
    // Only accept canonical tags: they also define the exact asset names.
    anyhow::ensure!(release.tag_name.starts_with('v') && release.tag_name.len() <= 32, "Tag de version invalide");
    https(&release.html_url)?;
    let setup = format!("NovyDeck-{}-Setup.exe", release.tag_name.trim_start_matches('v'));
    let sha = format!("{setup}.sha256");
    let asset = |name: &str| -> Result<&Asset> {
        let mut found = release.assets.iter().filter(|a| a.name == name);
        let item = found.next().context(format!("Fichier {name} absent de la version publiée"))?;
        anyhow::ensure!(found.next().is_none(), "Fichier {name} en double");
        https(&item.browser_download_url)?;
        Ok(item)
    };
    let exe = asset(&setup)?;
    let digest = asset(&sha)?;
    anyhow::ensure!(exe.size > 0 && exe.size <= MAX_SETUP as u64, "Taille de l'installateur invalide");
    anyhow::ensure!(digest.size > 0 && digest.size <= 4096, "Taille de l'empreinte invalide");
    Ok(Latest { version: release.tag_name, page: release.html_url, setup_url: exe.browser_download_url.clone(), setup_size: exe.size, sha_url: digest.browser_download_url.clone(), notes: release.body.unwrap_or_default() })
}
pub fn check() -> Result<Status> {
    let latest = parse_latest(&crate::https_get::get(LATEST_URL, 1024 * 1024)?)?;
    Ok(match compare(env!("CARGO_PKG_VERSION"), &latest.version)? {
        Ordering::Equal => Status::Current,
        Ordering::Less => Status::New(latest),
        Ordering::Greater => Status::Ahead,
    })
}
pub fn verify_sha256(bytes: &[u8], sha_text: &str) -> Result<()> {
    let token = sha_text.split_whitespace().next().context("Empreinte SHA-256 absente")?;
    anyhow::ensure!(token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()), "Empreinte SHA-256 invalide");
    anyhow::ensure!(format!("{:x}", Sha256::digest(bytes)).eq_ignore_ascii_case(token), "Empreinte SHA-256 différente : rien n'a été installé");
    Ok(())
}
pub fn installed_at(dir: &Path) -> bool { dir.join("unins000.exe").is_file() }
pub fn installed() -> bool {
    std::env::current_exe().ok().and_then(|exe| exe.parent().map(installed_at)).unwrap_or(false)
}

#[cfg(test)]
pub fn download(latest: &Latest) -> Result<PathBuf> { download_with_progress(latest, || {}) }
pub fn download_with_progress(latest: &Latest, verifying: impl FnOnce()) -> Result<PathBuf> {
    anyhow::ensure!(latest.setup_size > 0 && latest.setup_size <= MAX_SETUP as u64, "Taille de l'installateur invalide");
    https(&latest.setup_url)?;
    https(&latest.sha_url)?;
    let hash = crate::https_get::get(&latest.sha_url, 4096).context("Téléchargement de l'empreinte impossible")?;
    let hash = std::str::from_utf8(&hash).context("Empreinte SHA-256 illisible")?;
    let root = std::env::temp_dir().join("NovyDeck-update");
    fs::create_dir_all(&root).context("Création du dossier temporaire impossible")?;
    // A unique private subdirectory avoids replaying an older installer's file.
    let dir = tempfile::Builder::new().prefix("download-").tempdir_in(root).context("Dossier temporaire impossible")?;
    let path = dir.path().join("NovyDeck-Setup.exe");
    let result = (|| -> Result<()> {
        let bytes = crate::https_get::get(&latest.setup_url, MAX_SETUP).context("Téléchargement de l'installateur impossible")?;
        anyhow::ensure!(bytes.len() as u64 == latest.setup_size, "Taille de l'installateur différente : rien n'a été lancé");
        fs::write(&path, &bytes).context("Écriture de l'installateur impossible")?;
        verifying();
        verify_sha256(&bytes, hash)?;
        Ok(())
    })();
    if let Err(error) = result { let _ = fs::remove_file(&path); return Err(error); }
    let _ = dir.keep();
    Ok(path)
}

/// Arguments séparés : seul l'argument passé à /C est interprété par cmd.
pub fn installer_command(setup: &Path) -> (String, Vec<String>) {
    let path = setup.to_string_lossy();
    let script = format!("\"ping -n 4 127.0.0.1 >NUL & \"{path}\" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /relaunch=1\"");
    ("cmd.exe".into(), vec!["/D".into(), "/V:OFF".into(), "/C".into(), script])
}
/// Le shell et son enfant `ping` héritent d'une console cachée ; DETACHED_PROCESS
/// empêcherait cet héritage et laisserait `ping` ouvrir une console visible.
fn launch_flags() -> u32 { 0x0800_0000 } // CREATE_NO_WINDOW

pub fn launch(setup: &Path) -> Result<()> {
    use std::os::windows::process::CommandExt;
    anyhow::ensure!(setup.is_file(), "Installateur introuvable");
    let path = setup.to_string_lossy();
    anyhow::ensure!(!path.chars().any(|c| matches!(c, '"' | '%' | '!' | '^' | '\r' | '\n')), "Chemin temporaire incompatible avec cmd.exe");
    let (program, args) = installer_command(setup);
    // `args` échapperait les guillemets en \" que cmd.exe ne comprend pas : passer la ligne brute.
    let mut command = Command::new(program);
    for arg in &args { command.raw_arg(arg); }
    command.creation_flags(launch_flags())
        .spawn().context("Démarrage de l'installateur impossible")?;
    Ok(())
}

/// Banc de laboratoire (réseau réel, installe sur la machine) : jamais lancé par défaut.
#[cfg(test)] mod lab {
    #[test] #[ignore]
    fn lab_real_release_download_and_silent_update() {
        // 1. Vraie release GitHub : analyse, téléchargement et SHA-256 de l'installateur publié.
        let latest = match super::check().expect("API GitHub") {
            super::Status::New(l) => l,
            _ => {
                let body = crate::https_get::get("https://api.github.com/repos/HatiFlash/NovyDeck/releases/latest", 1 << 20).unwrap();
                super::parse_latest(&body).unwrap()
            }
        };
        let downloaded = super::download(&latest).expect("téléchargement + SHA-256");
        println!("LAB_DOWNLOADED {} {}", latest.version, std::fs::metadata(&downloaded).unwrap().len());
        // 2. Mise à jour silencieuse par la commande réelle avec l'installateur fourni (version locale).
        let Some(setup) = std::env::var_os("NOVYDECK_LAB_SETUP") else { return; };
        super::launch(std::path::Path::new(&setup)).expect("lancement détaché");
        println!("LAB_LAUNCHED");
    }
}

#[cfg(test)] mod tests {
    use super::*;
    fn release() -> serde_json::Value {
        serde_json::json!({"tag_name":"v0.8.14","html_url":"https://github.com/HatiFlash/NovyDeck/releases/tag/v0.8.14","draft":false,"prerelease":false,"assets":[
            {"name":"NovyDeck-0.8.14-Setup.exe","browser_download_url":"https://github.com/x/setup.exe","size":100},
            {"name":"NovyDeck-0.8.14-Setup.exe.sha256","browser_download_url":"https://github.com/x/setup.sha256","size":80}]})
    }
    #[test] #[ignore="requires interactive Windows desktop; uses a dummy .cmd, never an installer"]
    fn lab_update_wait_opens_no_visible_console() {
        use std::{thread,time::{Duration,Instant}};
        use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows,GetClassNameW,IsWindowVisible};
        use windows_sys::Win32::Foundation::{BOOL,HWND,LPARAM};
        unsafe extern "system" fn count(hwnd:HWND,data:LPARAM)->BOOL {
            if IsWindowVisible(hwnd)==0 { return 1; }
            let mut class=[0u16;80];let len=GetClassNameW(hwnd,class.as_mut_ptr(),80);
            if len>0 && matches!(String::from_utf16_lossy(&class[..len as usize]).as_str(),"ConsoleWindowClass"|"CASCADIA_HOSTING_WINDOW_CLASS") {
                *(data as *mut usize)+=1;
            }
            1
        }
        let consoles=||{let mut n=0usize;unsafe{EnumWindows(Some(count),&mut n as *mut _ as LPARAM)};n};
        let dir=tempfile::tempdir().unwrap();let setup=dir.path().join("fake-setup.cmd");let marker=dir.path().join("started.txt");
        fs::write(&setup,format!("@echo off\r\necho started > \"{}\"\r\n",marker.display())).unwrap();
        let before=consoles();launch(&setup).unwrap();let until=Instant::now()+Duration::from_secs(12);
        while Instant::now()<until && !marker.exists() {
            assert_eq!(consoles(),before,"visible console appeared during the update wait");
            thread::sleep(Duration::from_millis(30));
        }
        assert!(marker.exists(),"dummy setup did not run after the wait");
        assert_eq!(consoles(),before);
    }
    #[test] fn parses_published_release_only() {
        let mut value = release();
        assert_eq!(parse_latest(&serde_json::to_vec(&value).unwrap()).unwrap().setup_size, 100);
        value["draft"] = true.into(); assert!(parse_latest(&serde_json::to_vec(&value).unwrap()).is_err());
        value["draft"] = false.into(); value["prerelease"] = true.into(); assert!(parse_latest(&serde_json::to_vec(&value).unwrap()).is_err());
        value["prerelease"] = false.into(); value["assets"].as_array_mut().unwrap().pop();
        assert!(parse_latest(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    #[test] fn numeric_comparisons() {
        for (a,b) in [("0.8.9","0.8.13"),("0.8.13","v0.8.14"),("0.8.14","0.9.0"),("0.9.0","1.0.0")] {
            assert_eq!(compare(a,b).unwrap(), Ordering::Less);
        }
        assert_eq!(compare("v0.8.14","0.8.14").unwrap(), Ordering::Equal);
        assert!(compare("0.8.x","0.8.14").is_err());
    }
    #[test] fn digest_matches_or_refuses() {
        let hash = format!("{:X}", Sha256::digest(b"test"));
        verify_sha256(b"test", &hash).unwrap();
        verify_sha256(b"test", &format!("{hash}  fichier.exe\n")).unwrap();
        assert!(verify_sha256(b"incorrect", &hash).is_err());
        assert!(verify_sha256(b"test", "bad").is_err());
    }
    #[test] fn installation_detection_and_command() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!installed_at(dir.path()));
        fs::write(dir.path().join("unins000.exe"), b"").unwrap();
        assert!(installed_at(dir.path()));
        let (program, args) = installer_command(Path::new(r"C:\Some Folder\setup.exe"));
        assert_eq!(program, "cmd.exe");
        assert!(args.last().unwrap().contains(r#""C:\Some Folder\setup.exe" /VERYSILENT"#));
        assert!(args.last().unwrap().contains("/relaunch=1"));
    }
    #[test] fn launch_hides_console_for_cmd_and_ping() {
        assert_eq!(launch_flags(), 0x0800_0000); // CREATE_NO_WINDOW
        assert_eq!(launch_flags() & 0x0000_0008, 0); // never DETACHED_PROCESS
        let (_, args) = installer_command(Path::new(r"C:\Some Folder\setup.exe"));
        assert!(args.last().unwrap().contains("ping -n 4 127.0.0.1 >NUL &"));
    }
    #[test] fn release_notes_selects_language_and_strips_markdown() {
        let markdown = include_str!("../docs/RELEASE-0.8.14.md");
        let en = release_notes(markdown, "en");
        let fr = release_notes(markdown, "fr");
        assert!(en.contains("About → Vérifier les mises à jour"));
        assert!(fr.contains("À propos → Vérifier les mises à jour"));
        assert!(fr.contains("- Pour une copie installée"));
        assert!(fr.contains("Détail des contrôles"));
        assert!(!fr.contains("https://") && !fr.contains("**") && !fr.contains("##") && !fr.contains("English ·"));
        assert!(!en.contains("Pour une copie installée"));
        assert_eq!(release_notes("# Titre\n\n\n- [lien](https://example.org) et `code`", "fr"), "Titre\n\n- lien et code");
        assert_eq!(release_notes("texte sans sections", "fr"), "texte sans sections");
        let long = release_notes(&format!("## English\n{}", "é".repeat(5000)), "en");
        assert!(long.chars().count() <= 4000 && long.ends_with('…'));
    }
    #[test] fn missing_or_null_release_body_is_empty() {
        let mut value = release();
        assert!(parse_latest(&serde_json::to_vec(&value).unwrap()).unwrap().notes.is_empty());
        value["body"] = serde_json::Value::Null;
        assert!(parse_latest(&serde_json::to_vec(&value).unwrap()).unwrap().notes.is_empty());
        value["body"] = "## Français\n- Bonjour".into();
        assert!(parse_latest(&serde_json::to_vec(&value).unwrap()).unwrap().notes.contains("Bonjour"));
    }
}
