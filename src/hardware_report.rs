//! Rapport de support local : aucune transmission automatique, aucun numéro de série HID.
use std::{collections::VecDeque, fs, io, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use hidapi::HidApi;
use url::form_urlencoded;

const ISSUE: &str = "https://github.com/HatiFlash/NovyDeck/issues/new?template=hardware-report.yml";

pub struct Report {
    pub text: String,
    pub issue_url: String,
}

fn replace_ci(text: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() { return text.to_owned(); }
    let bytes = text.as_bytes();
    let pattern = needle.as_bytes();
    let mut result = String::new();
    let mut start = 0;
    let mut i = 0;
    while i + pattern.len() <= bytes.len() {
        if bytes[i..i + pattern.len()].eq_ignore_ascii_case(pattern) {
            result.push_str(&text[start..i]);
            result.push_str(replacement);
            i += pattern.len();
            start = i;
        } else { i += 1; }
    }
    result.push_str(&text[start..]);
    result
}

fn windows_users(text: &str) -> String {
    let mut result = String::new();
    let mut rest = text;
    while let Some(pos) = rest.as_bytes().windows(9).position(|w| w.eq_ignore_ascii_case(b"c:\\users\\") || w.eq_ignore_ascii_case(b"c:/users/")) {
        result.push_str(&rest[..pos]);
        let tail = &rest[pos + 9..];
        let name_end = tail.find(|c| c == '\\' || c == '/').unwrap_or(tail.len());
        if name_end == 0 { result.push_str(&rest[pos..pos + 9]); rest = tail; continue; }
        result.push_str("%USERPROFILE%");
        rest = &tail[name_end..];
    }
    result.push_str(rest);
    result
}

fn mask_emails(text: &str) -> String {
    let mut output = text.to_owned();
    loop {
        let Some(at) = output.find('@') else { break; };
        let left = output[..at].rfind(|c: char| !(c.is_ascii_alphanumeric() || "._%+-".contains(c))).map_or(0, |n| n + 1);
        let right = at + 1 + output[at + 1..].find(|c: char| !(c.is_ascii_alphanumeric() || ".-".contains(c))).unwrap_or(output.len() - at - 1);
        if left == at || right == at + 1 || !output[at + 1..right].contains('.') {
            // Skip this @ while leaving the remainder eligible for masking.
            let suffix = mask_emails(&output[at + 1..]);
            output.truncate(at + 1);
            output.push_str(&suffix);
            break;
        }
        output.replace_range(left..right, "<EMAIL>");
        // Already masked addresses have no @, but a malformed prefix could have one.
    }
    output
}

fn mask_serials(text: &str) -> String {
    text.lines().map(|line| {
        let mut line = line.to_owned();
        for label in ["série=", "serial=", "serial_number=", "serial number:", "numéro de série:"] {
            while let Some(at) = line.to_ascii_lowercase().find(label) {
                let begin = at + label.len();
                let end = line[begin..].find(|c| c == '|' || c == ';').map_or(line.len(), |i| begin + i);
                line.replace_range(begin..end, "<SERIAL> ");
                // Do not re-process the same label.
                line.replace_range(at..begin, "identifiant masqué=");
            }
        }
        line
    }).collect::<Vec<_>>().join("\n")
}

fn anonymize_with(text: &str, variables: &[(&str, &str)]) -> String {
    let mut out = text.to_owned();
    // Paths first, then user name and machine name (longest first, avoiding replacements inside tokens).
    for (name, token) in variables.iter().filter(|(name, _)| *name != "USERNAME" && *name != "COMPUTERNAME") {
        if let Some(value) = std::env::var(*name).ok().filter(|s| !s.is_empty()) {
            for candidate in [value.clone(), value.replace('\\', "/"), value.replace('/', "\\")] {
                out = replace_ci(&out, &candidate, token);
            }
        }
    }
    out = windows_users(&out);
    for (name, token) in variables.iter().filter(|(name, _)| *name == "USERNAME" || *name == "COMPUTERNAME") {
        if let Some(value) = std::env::var(*name).ok().filter(|s| s.len() >= 3) {
            out = replace_ci(&out, &value, token);
        }
    }
    mask_emails(&mask_serials(&out))
}

pub fn anonymize(text: &str) -> String {
    anonymize_with(text, &[("USERPROFILE", "%USERPROFILE%"), ("APPDATA", "%APPDATA%"),
        ("LOCALAPPDATA", "%LOCALAPPDATA%"), ("USERNAME", "<USER>"), ("COMPUTERNAME", "<COMPUTER>")])
}

fn windows_build() -> String {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let key = wide(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let name = wide("CurrentBuildNumber");
    let mut value = [0u16; 32];
    let mut size = (value.len() * 2) as u32;
    let result = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ,
        std::ptr::null_mut(), value.as_mut_ptr().cast(), &mut size) };
    if result != 0 { return "build indisponible".into(); }
    let end = value.iter().position(|&c| c == 0).unwrap_or(value.len());
    String::from_utf16_lossy(&value[..end])
}

fn issue_url(environment: &str) -> String {
    let title = "[Hardware] Problème avec mon deck";
    let mut end = environment.len().min(5900);
    while !environment.is_char_boundary(end) { end -= 1; }
    loop {
        let encoded = form_urlencoded::Serializer::new(String::new())
            .append_pair("title", title).append_pair("environment", &environment[..end]).finish();
        let url = format!("{ISSUE}&{encoded}");
        if url.len() <= 6000 { return url; }
        end = environment[..end].char_indices().next_back().map_or(0, |(i, _)| i);
    }
}

pub fn collect(config: &Path, status: &str, model: &str, connected: bool, logs: &VecDeque<String>) -> Report {
    let version = env!("CARGO_PKG_VERSION");
    let windows = windows_build();
    let mut hid = Vec::new();
    let mut inventory_error = None;
    match HidApi::new() {
        Ok(api) => for info in api.device_list() {
            hid.push(format!("{:04X}:{:04X} · {} · fabricant : {} · usage_page={:04X} usage={:04X} interface={}",
                info.vendor_id(), info.product_id(), info.product_string().unwrap_or("inconnu"),
                info.manufacturer_string().unwrap_or("inconnu"), info.usage_page(), info.usage(), info.interface_number()));
        },
        Err(e) => inventory_error = Some(format!("Inventaire HID indisponible : {e}")),
    }
    let summary = format!("NovyDeck {version} · Windows build {windows} · {}", hid.join(" ; "));
    let peripheral = crate::devices::report::latest(config).ok().flatten()
        .and_then(|path| fs::read_to_string(path).ok())
        .unwrap_or_else(|| "Journal périphérique indisponible".into());
    let text = format!("Rapport matériel NovyDeck\nVersion : {version}\nWindows : build {windows}\nArchitecture : {}\nPériphérique connu de NovyDeck : {model}\nConnecté : {}\nÉtat : {status}\n\nInventaire HID (aucun numéro de série) :\n{}\n\nJournal de l'application (300 lignes maximum) :\n{}\n\nJournal périphérique :\n{}\n",
        std::env::consts::ARCH, if connected { "oui" } else { "non" },
        inventory_error.unwrap_or_else(|| hid.join("\n")),
        logs.iter().rev().take(300).collect::<Vec<_>>().into_iter().rev().map(String::as_str).collect::<Vec<_>>().join("\n"), peripheral);
    let text = anonymize(&text);
    let environment = anonymize(&summary);
    let issue_url = issue_url(&environment);
    Report { text, issue_url }
}

fn timestamp(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}{month:02}{day:02}-{:02}{:02}{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

pub fn save(config: &Path, report: &Report) -> io::Result<PathBuf> {
    let dir = config.with_file_name("reports");
    fs::create_dir_all(&dir)?;
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let stamp = timestamp(secs);
    for n in 0..100 {
        let suffix = if n == 0 { String::new() } else { format!("-{n}") };
        let path = dir.join(format!("novydeck-report-{stamp}{suffix}.txt"));
        match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => { use std::io::Write; file.write_all(report.text.as_bytes())?; return Ok(path); }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "Trop de rapports pour cette seconde"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn issue_encoding_and_limit() {
        let url = issue_url(&"Écran + périphérique & modèle / ".repeat(1000));
        assert!(url.len() <= 6000);
        let parsed = url::Url::parse(&url).unwrap();
        assert_eq!(parsed.query_pairs().find(|(k, _)| k == "title").unwrap().1, "[Hardware] Problème avec mon deck");
        assert!(parsed.query_pairs().find(|(k, _)| k == "environment").unwrap().1.starts_with("Écran +"));
    }
    #[test] fn paths_email_and_serial_are_anonymized() {
        let text = "C:\\Users\\Alice\\test C:/Users/bob/Documents alice@example.org série=****1234 | serial=SECRET;";
        let safe = anonymize_with(text, &[]);
        assert!(!safe.contains("Alice") && !safe.contains("bob") && !safe.contains("example.org"));
        assert!(!safe.contains("1234") && !safe.contains("SECRET"));
        assert!(safe.contains("%USERPROFILE%") && safe.contains("<EMAIL>"));
    }
    #[test] fn report_filename_is_utc_and_stable() {
        assert_eq!(timestamp(0), "19700101-000000");
    }
}
