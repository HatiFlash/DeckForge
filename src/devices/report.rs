//! Rapport local de connexion HID. Aucune interrogation du matériel depuis les tests.
use std::{fs::{self, OpenOptions}, io::{self, Write}, path::{Path, PathBuf}, process::Command, sync::Mutex, time::Instant};
use hidapi::HidApi;

pub struct Report { file: Mutex<fs::File>, started: Instant }

fn system_command(program: &str, args: &[&str]) -> String {
    Command::new(program).args(args).output().ok()
        .filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|s| !s.is_empty()).unwrap_or_else(|| "indisponible".into())
}
fn stamp() -> String {
    system_command("powershell", &["-NoProfile", "-Command", "Get-Date -Format yyyyMMdd-HHmmss"])
}
fn rotate(dir: &Path) -> io::Result<()> {
    let mut files = fs::read_dir(dir)?.filter_map(Result::ok).map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("peripheriques-") && n.ends_with(".log")))
        .collect::<Vec<_>>();
    files.sort();
    let excess = files.len().saturating_sub(10);
    for p in files.into_iter().take(excess) { fs::remove_file(p)?; }
    Ok(())
}
pub fn directory(config: &Path) -> PathBuf { config.with_file_name("diagnostics") }
pub fn latest(config: &Path) -> io::Result<Option<PathBuf>> {
    let dir = directory(config);
    if !dir.exists() { return Ok(None); }
    Ok(fs::read_dir(dir)?.filter_map(Result::ok).map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("peripheriques-") && n.ends_with(".log")))
        .max())
}
pub fn mask_serial(serial: &str) -> String {
    let suffix: String = serial.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("****{suffix}")
}
pub fn sanitize(s: &str) -> String {
    let Ok(home) = std::env::var("USERPROFILE") else { return s.to_owned() };
    replace_home(s, &home)
}
fn replace_home(s: &str, home: &str) -> String {
    if home.is_empty() { return s.into(); }
    let mut result = String::new(); let mut rest = s;
    while let Some(pos) = rest.to_ascii_lowercase().find(&home.to_ascii_lowercase()) {
        let end = pos + home.len();
        result.push_str(&rest[..pos]); result.push_str("%USERPROFILE%"); rest = &rest[end..];
    }
    result.push_str(rest); result
}
pub fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}
impl Report {
    pub fn start(config: &Path) -> io::Result<Self> {
        let dir = directory(config); fs::create_dir_all(&dir)?;
        let file = OpenOptions::new().create(true).append(true).open(dir.join(format!("peripheriques-{}.log", stamp())))?;
        rotate(&dir)?;
        let report = Self { file: Mutex::new(file), started: Instant::now() };
        report.log(&format!("DeckForge {} git {} | Windows {}", env!("CARGO_PKG_VERSION"), env!("DECKFORGE_GIT_HASH"), system_command("cmd", &["/C", "ver"])));
        Ok(report)
    }
    pub fn log(&self, line: &str) {
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "+{:>8} ms | {}", self.started.elapsed().as_millis(), sanitize(line));
            let _ = file.flush();
        }
    }
    pub fn inventory(&self, api: &HidApi) {
        let list: Vec<_> = api.device_list().collect();
        self.log(&format!("Inventaire HID complet : {} interfaces", list.len()));
        for d in list {
            let model = super::MODELS.iter().find(|m| m.vid == d.vendor_id() && m.pid == d.product_id());
            let decision = match model {
                Some(m) if m.id == "fifine_d6" && d.usage_page() >= 0xff00 => "D6 choisi : usage_page vendeur; transport D6 actif".into(),
                Some(m) if m.id == "fifine_d6" => "D6 écarté : usage_page hors plage vendeur".into(),
                Some(m) if m.family == super::Family::Mirabox && matches!(m.protocol, super::Protocol::MiraboxV1 | super::Protocol::MiraboxV2) && d.usage_page() >= 0xff00 =>
                    format!("{} expérimental : pilote Mirabox disponible, matériel non vérifié", m.name),
                Some(m) => format!("{} expérimental : reconnu dans le catalogue, transport non activé pour cette interface/protocole", m.name),
                None => "Écarté : VID/PID absent du catalogue".into(),
            };
            self.log(&format!("VID={:04X} PID={:04X} usage_page={:04X} usage={:04X} interface={} fabricant={:?} produit={:?} série={} | {}",
                d.vendor_id(), d.product_id(), d.usage_page(), d.usage(), d.interface_number(),
                d.manufacturer_string(), d.product_string(), d.serial_number().map(mask_serial).unwrap_or_else(|| "absent".into()), decision));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn redact_serial_and_home() {
        assert_eq!(mask_serial("AB123456"), "****3456");
        assert_eq!(mask_serial("xy"), "****xy");
        assert_eq!(replace_home(r"C:\Users\Hatim\Images\x", r"c:\users\hatim"), r"%USERPROFILE%\Images\x");
    }
    #[test] fn hex_format() { assert_eq!(hex(&[0, 1, 0xfe, 0xff]), "00 01 FE FF"); }
    #[test] fn rotation_keeps_ten_latest() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..13 { fs::write(dir.path().join(format!("peripheriques-20260101-{:06}.log", i)), "ok").unwrap(); }
        fs::write(dir.path().join("unrelated.txt"), "ok").unwrap();
        rotate(dir.path()).unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 11);
        assert!(!dir.path().join("peripheriques-20260101-000000.log").exists());
        assert!(dir.path().join("peripheriques-20260101-000012.log").exists());
    }
}
