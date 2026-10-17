use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    embed_icon();
    for path in [".git/HEAD", ".git/refs"] {
        if std::path::Path::new(path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    // A detached worktree may have .git as a file; never make Git a build requirement.
    let hash = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|hash| hash.trim().to_string())
        .filter(|hash| !hash.is_empty())
        .unwrap_or_else(|| "inconnu".into());
    println!("cargo:rustc-env=DECKFORGE_GIT_HASH={hash}");
    // UTC calendar date, without a dependency or an external date command.
    let days = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() / 86_400;
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    if month <= 2 { year += 1; }
    println!("cargo:rustc-env=DECKFORGE_BUILD_DATE={year:04}-{month:02}-{day:02}");
}

/// Embeds res/branding/deckforge.ico as the executable icon (Explorer, taskbar shortcut).
/// Optional: without windres the build continues with the default icon.
fn embed_icon() {
    println!("cargo:rerun-if-changed=res/branding/deckforge.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") { return; }
    let Ok(out) = std::env::var("OUT_DIR") else { return };
    let icon = std::path::Path::new("res/branding/deckforge.ico");
    let Ok(icon) = icon.canonicalize() else { return };
    let rc = std::path::Path::new(&out).join("deckforge-icon.rc");
    let obj = std::path::Path::new(&out).join("deckforge-icon.o");
    let path = icon.to_string_lossy().trim_start_matches(r"\\?\").replace('\\', "/");
    if std::fs::write(&rc, format!("1 ICON \"{path}\"\n")).is_err() { return; }
    let ok = Command::new("windres").args(["-i"]).arg(&rc).args(["-O", "coff", "-o"]).arg(&obj).status().map(|s| s.success()).unwrap_or(false);
    if ok {
        println!("cargo:rustc-link-arg-bins={}", obj.display());
    }
}
