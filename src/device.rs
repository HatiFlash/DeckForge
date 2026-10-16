use crate::{model::Config, Event};
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use anyhow::{Context, Result};
use hidapi::{HidApi, HidDevice};
use image::{Rgb, RgbImage};
use std::{
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    time::{Duration, Instant},
};

pub enum Command {
    Connect(Config, u64),
    Update(Config, u64),
    Disconnect,
}
pub fn spawn(tx: Sender<Event>) -> Sender<Command> {
    let (cmd, rx) = mpsc::channel();
    std::thread::spawn(move || worker(rx, tx));
    cmd
}
fn worker(rx: Receiver<Command>, tx: Sender<Event>) {
    let mut wanted = false;
    let mut config = Config::default();
    let mut revision = 0;
    let mut deck: Option<Deck> = None;
    let mut retry = Instant::now();
    let mut beat = Instant::now();
    let mut last_press = [Instant::now() - Duration::from_secs(1); 15];
    loop {
        let mut update = false;
        loop {
            match rx.try_recv() {
                Ok(Command::Connect(c, r)) => {
                    config = c;
                    revision = r;
                    wanted = true;
                    update = true;
                    retry = Instant::now();
                }
                Ok(Command::Update(c, r)) => {
                    config = c;
                    revision = r;
                    update = true;
                }
                Ok(Command::Disconnect) => {
                    wanted = false;
                    deck = None;
                    let _ = tx.send(Event::Device(false, "D6 déconnecté de DeckForge".into()));
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        if wanted && deck.is_none() && Instant::now() >= retry {
            retry = Instant::now() + Duration::from_secs(3);
            let result = if vendor_running() {
                Err(anyhow::anyhow!(
                    "Ferme FIFINE Control Deck, y compris son icône près de l’horloge"
                ))
            } else {
                Deck::open()
            };
            match result {
                Ok(d) => {
                    deck = Some(d);
                    update = true;
                    beat = Instant::now();
                }
                Err(e) => {
                    let _ = tx.send(Event::Device(false, format!("En attente · {e}")));
                }
            }
        }
        let mut failed = None;
        if let Some(d) = &deck {
            if update {
                match d.apply(&config) {
                    Ok(()) => {
                        let _ = tx.send(Event::Device(
                            true,
                            format!("D6 connecté · 15 touches · rapports {} octets", d.packet),
                        ));
                        let _ = tx.send(Event::Applied(revision));
                    }
                    Err(e) => failed = Some(e.to_string()),
                }
            }
            if failed.is_none() && beat.elapsed() >= Duration::from_secs(8) {
                if vendor_running() {
                    failed = Some(
                        "FIFINE a été relancé : connexion suspendue pour éviter les conflits"
                            .into(),
                    );
                } else if let Err(e) = d.command(b"CONNECT") {
                    failed = Some(e.to_string());
                }
                beat = Instant::now();
            }
            if failed.is_none() {
                let mut data = [0u8; 2048];
                match d.hid.read_timeout(&mut data, 30) {
                    Ok(n) if n > 0 => {
                        if let Some(mut k) = decode_press(&data[..n]) {
                            if config.flip_input_rows {
                                k = display_index(k);
                            }
                            if last_press[k].elapsed() >= Duration::from_millis(150) {
                                last_press[k] = Instant::now();
                                let _ = tx.send(Event::Key(k, Instant::now()));
                            }
                        }
                    }
                    Err(e) => failed = Some(e.to_string()),
                    _ => {}
                }
            }
        } else {
            std::thread::sleep(Duration::from_millis(50));
        }
        if let Some(error) = failed {
            deck = None;
            let _ = tx.send(Event::Device(
                false,
                format!("Connexion interrompue · {error}"),
            ));
        }
    }
}
pub fn vendor_running() -> bool {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::*,
    };
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut entry);
        let mut found = false;
        while ok != 0 {
            let n = entry
                .szExeFile
                .iter()
                .position(|&u| u == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..n]).to_lowercase();
            if name.contains("fifine") && name.contains("deck") {
                found = true;
                break;
            }
            ok = Process32NextW(snap, &mut entry);
        }
        CloseHandle(snap);
        found
    }
}
pub fn display_index(k: usize) -> usize {
    (2 - k / 5) * 5 + k % 5
}
pub fn decode_press(data: &[u8]) -> Option<usize> {
    let d = if data.first() == Some(&0) {
        &data[1..]
    } else {
        data
    };
    if d.len() < 11
        || &d[..3] != b"ACK"
        || &d[5..7] != b"OK"
        || !(1..=15).contains(&d[9])
        || d[10] != 1
    {
        return None;
    }
    Some(d[9] as usize - 1)
}
// Reconstruct packet length from the actual HID descriptor; never guess 512/1024.
fn output_report(desc: &[u8]) -> Result<(usize, u8)> {
    let (mut size, mut count, mut rid) = (0usize, 0usize, 0u8);
    let mut globals = Vec::new();
    let mut outputs = std::collections::BTreeMap::<u8, usize>::new();
    let mut i = 0;
    while i < desc.len() {
        let prefix = desc[i];
        i += 1;
        if prefix == 0xFE {
            anyhow::ensure!(i + 2 <= desc.len(), "Descripteur HID tronqué");
            let len = desc[i] as usize;
            i += 2 + len;
            continue;
        }
        let len = match prefix & 3 {
            3 => 4,
            n => n as usize,
        };
        anyhow::ensure!(i + len <= desc.len(), "Descripteur HID tronqué");
        let mut value = 0usize;
        for j in 0..len {
            value |= (desc[i + j] as usize) << (8 * j);
        }
        i += len;
        match prefix & 0xFC {
            0x74 => size = value,
            0x94 => count = value,
            0x84 => rid = value as u8,
            0xA4 => globals.push((size, count, rid)),
            0xB4 => {
                if let Some(g) = globals.pop() {
                    (size, count, rid) = g;
                }
            }
            0x90 => {
                *outputs.entry(rid).or_default() +=
                    size.checked_mul(count).context("Rapport HID trop long")?;
            }
            _ => {}
        }
    }
    let (&rid, &bits) = outputs
        .iter()
        .max_by_key(|(_, bits)| **bits)
        .context("Aucun rapport de sortie HID")?;
    let length = bits.div_ceil(8);
    anyhow::ensure!(
        [512, 1024].contains(&length),
        "Taille de rapport non prise en charge : {length}"
    );
    Ok((length, rid))
}
struct Deck {
    hid: HidDevice,
    packet: usize,
    rid: u8,
}
impl Deck {
    fn open() -> Result<Self> {
        let api = HidApi::new()?;
        let info = api
            .device_list()
            .find(|d| {
                d.vendor_id() == 0x3142 && d.product_id() == 0x0060 && d.usage_page() >= 0xFF00
            })
            .context("D6 3142:0060 introuvable")?;
        let hid = info.open_device(&api).context("Accès au D6 impossible")?;
        let mut descriptor = [0u8; 4096];
        let n = hid.get_report_descriptor(&mut descriptor)?;
        let (packet, rid) = output_report(&descriptor[..n])?;
        Ok(Self { hid, packet, rid })
    }
    fn report(&self, payload: &[u8]) -> Result<()> {
        anyhow::ensure!(payload.len() <= self.packet, "Rapport trop long");
        let mut data = vec![0u8; self.packet + 1];
        data[0] = self.rid;
        data[1..1 + payload.len()].copy_from_slice(payload);
        // windows-native reports 0 for a synchronously completed WriteFile.
        let n = self.hid.write(&data)?;
        anyhow::ensure!(
            n == 0 || n == data.len(),
            "Rapport HID partiellement envoyé"
        );
        Ok(())
    }
    fn command(&self, tail: &[u8]) -> Result<()> {
        let mut data = b"CRT\0\0".to_vec();
        data.extend(tail);
        self.report(&data)
    }
    fn apply(&self, c: &Config) -> Result<()> {
        c.validate()?;
        // Render everything first: a bad PNG leaves the device's previous screen intact.
        let fontpath = std::path::PathBuf::from(
            std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()),
        )
        .join("Fonts/segoeui.ttf");
        let font = FontArc::try_from_vec(std::fs::read(fontpath)?)
            .map_err(|_| anyhow::anyhow!("Police Windows illisible"))?;
        let images = c
            .keys()
            .iter()
            .map(|k| render_key(k, c.image_size, &font))
            .collect::<Result<Vec<_>>>()?;
        self.command(b"DIS")?;
        self.command(&[b'L', b'I', b'G', 0, 0, c.brightness])?;
        self.report(b"HAN")?;
        self.command(&[b'C', b'L', b'E', 0, 0, 0, 0xFF])?;
        for (idx, jpeg) in images.iter().enumerate() {
            anyhow::ensure!(jpeg.len() <= 65535, "Image JPEG trop lourde");
            self.command(&[
                b'B',
                b'A',
                b'T',
                0,
                0,
                (jpeg.len() >> 8) as u8,
                jpeg.len() as u8,
                display_index(idx) as u8 + 1,
            ])?;
            for chunk in jpeg.chunks(self.packet) {
                self.report(chunk)?;
            }
        }
        self.command(b"ULEND")?;
        self.command(b"STP")?;
        Ok(())
    }
}
fn render_key(k: &crate::model::Key, size: u32, font: &FontArc) -> Result<Vec<u8>> {
    let mut image = if k.image.is_empty() {
        RgbImage::from_pixel(size, size, Rgb(k.color))
    } else {
        let reader = image::ImageReader::open(&k.image)?.with_guessed_format()?;
        let mut reader = reader;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        reader
            .decode()
            .with_context(|| format!("Image invalide pour {}", k.label))?
            .resize_to_fill(size, size, image::imageops::FilterType::Lanczos3)
            .into_rgb8()
    };
    let has_icon = k.image.is_empty() && !k.steps.is_empty();
    if has_icon {
        crate::icons::draw(&mut image, crate::icons::kind(k));
    }
    if !k.label.is_empty() {
        let chars = k.label.chars().take(48).collect::<Vec<_>>();
        let lines = chars
            .chunks(12)
            .take(if has_icon { 2 } else { 3 })
            .map(|chunk| chunk.iter().collect::<String>())
            .collect::<Vec<_>>();
        let scaled = font.as_scaled(PxScale::from(size as f32 * 0.14));
        let height = scaled.height();
        let y0 = if has_icon {
            size as f32 * 0.59
        } else if k.image.is_empty() {
            (size as f32 - height * lines.len() as f32) / 2.0
        } else {
            size as f32 - height * lines.len() as f32 - 4.0
        };
        if !k.image.is_empty() {
            for y in (y0.max(0.0) as u32)..size {
                for x in 0..size {
                    let pixel = image.get_pixel_mut(x, y);
                    for ch in &mut pixel.0 {
                        *ch = (*ch as f32 * 0.25) as u8;
                    }
                }
            }
        }
        for (line_idx, line) in lines.iter().enumerate() {
            let width: f32 = line
                .chars()
                .map(|c| scaled.h_advance(scaled.glyph_id(c)))
                .sum();
            let mut x = (size as f32 - width) / 2.0;
            let y = y0 + line_idx as f32 * height + scaled.ascent();
            for c in line.chars() {
                let id = scaled.glyph_id(c);
                let glyph = id.with_scale_and_position(scaled.scale(), ab_glyph::point(x, y));
                if let Some(outline) = font.outline_glyph(glyph) {
                    let bounds = outline.px_bounds();
                    outline.draw(|gx, gy, coverage| {
                        let px = bounds.min.x as i32 + gx as i32;
                        let py = bounds.min.y as i32 + gy as i32;
                        if px >= 0 && py >= 0 && px < size as i32 && py < size as i32 {
                            let p = image.get_pixel_mut(px as u32, py as u32);
                            for channel in &mut p.0 {
                                *channel =
                                    (*channel as f32 * (1.0 - coverage) + 255.0 * coverage) as u8;
                            }
                        }
                    });
                }
                x += scaled.h_advance(id);
            }
        }
    }
    let image = image::imageops::rotate180(&image);
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 88).encode(
        &image,
        size,
        size,
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(bytes)
}
pub fn diagnostic() {
    let result = (|| -> Result<serde_json::Value> {
        let api = HidApi::new()?;
        let mut entries = Vec::new();
        for info in api
            .device_list()
            .filter(|d| d.vendor_id() == 0x3142 && d.product_id() == 0x0060)
        {
            let mut v = serde_json::json!({"vid":"3142", "pid":"0060", "usage_page":info.usage_page(), "usage":info.usage(), "interface":info.interface_number(), "product":info.product_string()});
            if info.usage_page() >= 0xFF00 {
                match info.open_device(&api).and_then(|h| {
                    let mut b = [0; 4096];
                    h.get_report_descriptor(&mut b).map(|n| b[..n].to_vec())
                }) {
                    Ok(desc) => {
                        v["descriptor"] = serde_json::json!(desc);
                        v["output"] = serde_json::json!(output_report(&desc)
                            .map(|(n, r)| format!("{n} bytes, report id {r}"))
                            .unwrap_or_else(|e| e.to_string()));
                    }
                    Err(e) => v["error"] = serde_json::json!(e.to_string()),
                }
            }
            entries.push(v);
        }
        Ok(
            serde_json::json!({"vendor_app_running":vendor_running(), "devices": entries, "read_only":true}),
        )
    })();
    let value = result.unwrap_or_else(|e| serde_json::json!({"error":e.to_string()}));
    let path = std::env::current_exe()
        .unwrap()
        .with_file_name("diagnostic.json");
    let _ = std::fs::write(path, serde_json::to_vec_pretty(&value).unwrap());
}

// Explicit opt-in integration test: numbered screens and input capture only.
// No action engine is created, so pressing a key cannot affect other apps.
pub fn hardware_test() {
    let path = std::env::current_exe()
        .unwrap()
        .with_file_name("hardware-test.json");
    let mut report =
        serde_json::json!({"state":"starting","actions_enabled":false,"presses":[],"packets":[]});
    let result = (|| -> Result<()> {
        anyhow::ensure!(!vendor_running(), "FIFINE est encore en cours d’exécution");
        let deck = Deck::open()?;
        let mut config = Config::default();
        if std::env::args().any(|a| a == "--112") {
            config.image_size = 112;
        }
        for (idx, key) in config.profiles[0].keys.iter_mut().enumerate() {
            key.label = format!("TEST {:02}", idx + 1);
            key.steps.clear();
            key.color = match idx / 5 {
                0 => [92, 64, 180],
                1 => [25, 110, 105],
                _ => [150, 70, 45],
            };
        }
        deck.apply(&config)?;
        report["state"] = serde_json::json!("listening");
        report["output_packet_bytes"] = serde_json::json!(deck.packet);
        report["image_size"] = serde_json::json!(config.image_size);
        std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
        let start = Instant::now();
        let mut beat = Instant::now();
        while start.elapsed() < Duration::from_secs(180) {
            let mut data = [0u8; 2048];
            let n = deck.hid.read_timeout(&mut data, 50)?;
            if n > 0 {
                if report["packets"].as_array().unwrap().len() < 100 {
                    report["packets"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!(data[..n.min(32)].to_vec()));
                }
                if let Some(k) = decode_press(&data[..n]) {
                    report["presses"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!({"key":k+1,"time_ms":start.elapsed().as_millis()}));
                }
                std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
                if report["presses"].as_array().unwrap().len() >= 3 {
                    break;
                }
            }
            if beat.elapsed() > Duration::from_secs(8) {
                deck.command(b"CONNECT")?;
                beat = Instant::now();
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => report["state"] = serde_json::json!("completed"),
        Err(e) => {
            report["state"] = serde_json::json!("error");
            report["error"] = serde_json::json!(e.to_string());
        }
    }
    let _ = std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap());
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_rows_and_input_are_independent() {
        assert_eq!(display_index(0), 10);
        assert_eq!(display_index(14), 4);
        for k in 0..15 {
            assert_eq!(display_index(display_index(k)), k);
        }
    }
    #[test]
    fn decoder_ignores_refresh_release_and_bad_packets() {
        let mut data = [0; 12];
        data[..3].copy_from_slice(b"ACK");
        data[5..7].copy_from_slice(b"OK");
        assert_eq!(decode_press(&data), None);
        data[9] = 15;
        data[10] = 1;
        assert_eq!(decode_press(&data), Some(14));
        data[5..7].copy_from_slice(b"TF");
        assert_eq!(decode_press(&data), None);
        data[5..7].copy_from_slice(b"OK");
        data[10] = 2;
        assert_eq!(decode_press(&data), None);
        // Captured from this Windows D6: release is 0, not 2.
        data[10] = 0;
        assert_eq!(decode_press(&data), None);
        data[9] = 16;
        assert_eq!(decode_press(&data), None);
        assert_eq!(decode_press(&[0]), None);
    }
    #[test]
    fn descriptor_sizes_are_not_hardcoded() {
        assert_eq!(
            output_report(&[0x75, 8, 0x96, 0, 2, 0x91, 2]).unwrap(),
            (512, 0)
        );
        assert_eq!(
            output_report(&[0x85, 1, 0x75, 8, 0x96, 0, 4, 0x91, 2]).unwrap(),
            (1024, 1)
        );
        assert!(output_report(&[0x75]).is_err());
    }
}
