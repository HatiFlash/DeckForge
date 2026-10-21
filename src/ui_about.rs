//! About dialog and private-repository update check. No network activity on the UI thread.
use std::{process::{Command, Stdio}, thread, time::{Duration, Instant}};

pub(super) const HASH: &str = env!("DECKFORGE_GIT_HASH");
pub(super) const DATE: &str = env!("DECKFORGE_BUILD_DATE");
const REPOSITORY: &str = "repos/HatiFlash/NovyDeck";

pub(super) fn information() -> String {
    format!("NovyDeck {}\nCompilation : {} ({})", env!("CARGO_PKG_VERSION"), HASH, DATE)
}

// Pure decision logic: never equate a failed or malformed response to an up-to-date build.
pub(super) fn decide(hash: &str, latest: Result<&str, &str>, comparison: Result<&str, &str>) -> String {
    if hash == "inconnu" || hash.len() < 7 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return "Compilation sans identifiant".into();
    }
    let latest = match latest {
        Ok(value) if value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()) => value,
        Ok(_) => return "Réponse GitHub invalide".into(),
        Err(error) => return error.into(),
    };
    if latest.starts_with(hash) {
        return "À jour : cette compilation correspond à la dernière version sur GitHub".into();
    }
    let comparison = match comparison {
        Ok(value) => value.trim(),
        Err(error) => return error.into(),
    };
    let mut fields = comparison.split_whitespace();
    match (fields.next(), fields.next(), fields.next()) {
        (Some("ahead"), Some(n), None) if n.parse::<u64>().is_ok_and(|n| n > 0) =>
            format!("Nouvelle version disponible : {n} modification(s) sur GitHub"),
        (Some("behind"), Some(n), None) if n.parse::<u64>().is_ok() =>
            "Cette compilation est plus récente que GitHub".into(),
        (Some("diverged"), Some(n), None) if n.parse::<u64>().is_ok() =>
            "Versions différentes".into(),
        _ => "Réponse GitHub invalide".into(),
    }
}

fn gh(args: &[&str], deadline: Instant) -> Result<String, &'static str> {
    use std::os::windows::process::CommandExt;
    let mut child = Command::new("gh")
        .args(args).creation_flags(0x0800_0000)
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e| if e.kind() == std::io::ErrorKind::NotFound {
            "GitHub CLI absent"
        } else { "Réseau indisponible ou délai dépassé" })?;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().map_err(|_| "Réseau indisponible ou délai dépassé")?;
                if !status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
                    // Never expose stderr: it may contain repository details or credentials.
                    return Err(if stderr.contains("authentication") || stderr.contains("authenticate")
                        || stderr.contains("not logged") || stderr.contains("http 401")
                        || stderr.contains("http 403") || stderr.contains("http 404")
                        || stderr.contains("not found") || stderr.contains("login") {
                        "GitHub CLI non connecté (dépôt privé)"
                    } else { "Réseau indisponible ou délai dépassé" });
                }
                return String::from_utf8(output.stdout).map_err(|_| "Réponse GitHub invalide");
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Réseau indisponible ou délai dépassé");
            }
        }
    }
}

pub(super) fn check() -> String {
    if HASH == "inconnu" || HASH.len() < 7 || !HASH.bytes().all(|b| b.is_ascii_hexdigit()) {
        return "Compilation sans identifiant".into();
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    let latest = gh(&["api", &format!("{REPOSITORY}/commits/main"), "--jq", ".sha"], deadline);
    let latest = match latest {
        Ok(value) => value,
        Err(error) => return error.into(),
    };
    if latest.trim().starts_with(HASH) {
        return decide(HASH, Ok(latest.trim()), Ok(""));
    }
    let compare = gh(&["api", &format!("{REPOSITORY}/compare/{HASH}...main"),
        "--jq", ".status + \" \" + (.ahead_by|tostring)"], deadline);
    decide(HASH, Ok(latest.trim()), compare.as_deref().map_err(|error| *error))
}

#[cfg(test)]
mod tests {
    use super::*;
    const LOCAL: &str = "4f3cab7";
    const REMOTE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    #[test]
    fn update_decisions_offline() {
        assert!(decide(LOCAL, Ok("4f3cab7aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), Ok("" )).starts_with("À jour"));
        assert_eq!(decide(LOCAL, Ok(REMOTE), Ok("ahead 3")), "Nouvelle version disponible : 3 modification(s) sur GitHub");
        assert_eq!(decide(LOCAL, Ok(REMOTE), Ok("behind 2")), "Cette compilation est plus récente que GitHub");
        assert_eq!(decide(LOCAL, Ok(REMOTE), Ok("diverged 1")), "Versions différentes");
        assert_eq!(decide("inconnu", Ok(REMOTE), Ok("ahead 1")), "Compilation sans identifiant");
        assert_eq!(decide(LOCAL, Err("GitHub CLI absent"), Ok("ahead 1")), "GitHub CLI absent");
        assert_eq!(decide(LOCAL, Err("GitHub CLI non connecté (dépôt privé)"), Ok("ahead 1")), "GitHub CLI non connecté (dépôt privé)");
        assert_eq!(decide(LOCAL, Ok(REMOTE), Err("Réseau indisponible ou délai dépassé")), "Réseau indisponible ou délai dépassé");
        assert_eq!(decide(LOCAL, Ok("bad"), Ok("ahead 1")), "Réponse GitHub invalide");
        assert_eq!(decide(LOCAL, Ok(REMOTE), Ok("ahead 0")), "Réponse GitHub invalide");
    }
}
