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

/// Ressources Windows de l'exécutable : icône, VERSIONINFO (éditeur, produit, version, copyright)
/// et manifeste d'application. Compilées avec rc.exe du Windows SDK via `winresource` (fonctionne
/// avec la chaîne MSVC et sur les runners GitHub). Un exécutable sans métadonnées est davantage
/// suspecté par l'analyse heuristique de Microsoft Defender.
fn embed_icon() {
    println!("cargo:rerun-if-changed=res/branding/deckforge.ico");
    println!("cargo:rerun-if-changed=res/windows/deckforge.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    #[cfg(windows)]
    {
        let version = env!("CARGO_PKG_VERSION");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("res/branding/deckforge.ico")
            .set_manifest_file("res/windows/deckforge.manifest")
            .set("ProductName", "DeckForge")
            .set("FileDescription", "DeckForge - contrôleur de macros pour FIFINE AmpliGame D6")
            .set("CompanyName", "HatiFlash")
            .set("LegalCopyright", "© 2026 HatiFlash - licence MIT")
            .set("InternalName", "DeckForge")
            .set("Comments", "https://github.com/HatiFlash/DeckForge")
            .set("ProductVersion", version)
            .set("FileVersion", version)
            .set_language(0x040c);
        if let Err(e) = res.compile() {
            println!("cargo:warning=Ressources Windows non intégrées (rc.exe introuvable ?) : {e}");
        }
    }
}
