use crate::key_render::for_device as render_key_or_default;
#[cfg(test)]
use crate::key_render::jpeg as render_key;
use crate::{model::Config, Event};
use crate::devices::{self, Detection, DeviceModel, Family, ImageFormat, Protocol};
use crate::devices::report::{self, Report};
use crate::devices::elgato;
use std::{path::PathBuf, sync::Arc, cell::Cell};
#[cfg(test)]
use ab_glyph::FontArc;
use anyhow::{Context, Result};
use hidapi::{HidApi, HidDevice};
use std::{
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    time::{Duration, Instant},
};

/// Pure decision: no upload during apply, one frame per turn, fair round-robin.
fn next_animation_frame(now: Instant, applying: bool, enabled: bool, fps: u8,
    lengths: &[usize; 15], due: &[Instant; 15], last_per_key: &[Instant; 15],
    last_global: Instant, cursor: usize) -> Option<usize> {
    if applying || !enabled || now.saturating_duration_since(last_global) < Duration::from_millis(34) {
        return None;
    }
    let interval = Duration::from_millis(1000 / fps.max(1) as u64);
    (0..15).map(|n| (cursor + n) % 15).find(|&i| lengths[i] > 1
        && now >= due[i] && now.saturating_duration_since(last_per_key[i]) >= interval)
}

pub enum Command {
    Connect(Config, u64),
    Update(Config, u64),
    Brightness(u8),
    KeyImage(usize, Vec<u8>),
    Disconnect,
    TestStart,
    TestStop,
    TestBrightness,
}
pub fn spawn(tx: Sender<Event>, config_path: PathBuf) -> Sender<Command> {
    let (cmd, rx) = mpsc::channel();
    std::thread::spawn(move || worker(rx, tx, config_path));
    cmd
}
fn worker(rx: Receiver<Command>, tx: Sender<Event>, config_path: PathBuf) {
    // Rapport dès le démarrage, même si le D6 n’est pas connecté.
    if let Ok(report) = Report::start(&config_path) {
        match HidApi::new() { Ok(api) => report.inventory(&api), Err(e) => report.log(&format!("Inventaire HID indisponible : {e}")) }
    } else { let _ = tx.send(Event::Log("Rapport périphérique : création impossible".into())); }
    let mut test_running = false;
    let mut test_brightness_at: Option<Instant> = None;
    let mut wanted = false;
    let mut config = Config::default();
    let mut revision = 0;
    let mut deck: Option<Deck> = None;
    let mut retry = Instant::now();
    let mut beat = Instant::now();
    let mut last_press = [Instant::now() - Duration::from_secs(1); 32];
    let (animation_tx, animation_rx) = mpsc::channel::<(u64, Vec<Vec<crate::key_render::AnimatedFrame>>)>() ;
    let mut animation_generation = 0u64;
    let mut animations: Vec<Vec<crate::key_render::AnimatedFrame>> = Vec::new();
    let mut next_frame = [1usize; 15];
    let mut due = [Instant::now(); 15];
    let mut last_frame = Instant::now() - Duration::from_secs(1);
    let mut last_per_key = [Instant::now() - Duration::from_secs(1); 15];
    let mut cursor = 0usize;
    let mut key_images: [Option<Vec<u8>>;32] = std::array::from_fn(|_|None);
    loop {
        let mut update = false;
        let mut brightness = None;
        loop {
            match rx.try_recv() {
                Ok(Command::TestStart) => {
                    if let Some(d) = &deck {
                        test_running = true;
                        test_brightness_at = None;
                        let result = (|| -> Result<()> {
                            let font = crate::key_render::font()?;
                            for idx in 0..d.driver.model().keys as usize {
                                let mut key = crate::model::Key::default();
                                key.label = (idx+1).to_string();
                                key.title_size = 46;
                                key.color = [20, 56, 90];
                                key.background = true;
                                let image = crate::key_render::jpeg(&key, d.driver.model().image_w.max(60) as u32, &font)?;
                                d.set_key_image(idx, &image, frame_end())?;
                            }
                            Ok(())
                        })();
                        match result {
                            Ok(()) => { let _=tx.send(Event::HardwareTestStatus("Images numérotées : OK. Appuie sur les touches dans l’ordre.".into())); },
                            Err(e) => { test_running=false; update=true; let _=tx.send(Event::HardwareTestStatus(format!("Images numérotées : ÉCHEC — {e}"))); }
                        }
                    } else { let _=tx.send(Event::HardwareTestStatus("ÉCHEC : périphérique non connecté".into())); }
                }
                Ok(Command::TestBrightness) => {
                    if test_running {
                        if let Some(d)=&deck {
                            match d.set_brightness(30) {
                                Ok(()) => { test_brightness_at=Some(Instant::now()); let _=tx.send(Event::HardwareTestStatus("Luminosité 30 % : commande OK".into())); },
                                Err(e) => { let _=tx.send(Event::HardwareTestStatus(format!("ÉCHEC luminosité 30 % : {e}"))); }
                            }
                        }
                    }
                }
                Ok(Command::TestStop) => { if test_running { test_running=false; test_brightness_at=None; update=true; let _=tx.send(Event::HardwareTestStatus("Test arrêté ; affichage normal restauré".into())); } }
                Ok(Command::Connect(c, r)) => {
                    test_running = false;
                    config = c;
                    revision = r;
                    wanted = true;
                    update = true;
                    retry = Instant::now();
                }
                Ok(Command::Update(c, r)) => {
                    config = c;
                    revision = r;
                    update = !test_running;
                }
                Ok(Command::Brightness(value)) => brightness = Some(value.min(100)),
                Ok(Command::KeyImage(index,jpeg)) => { if index<32 && jpeg.len()<=100_000 {key_images[index]=Some(jpeg);} },
                Ok(Command::Disconnect) => {
                    wanted = false;
                    test_running = false;
                    deck = None;
                    key_images.fill(None);
                    let _ = tx.send(Event::Device(false, "Périphérique déconnecté de DeckForge".into()));
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        if wanted && deck.is_none() && Instant::now() >= retry {
            retry = Instant::now() + Duration::from_secs(3);
            let report = match Report::start(&config_path) {
                Ok(report) => { let report = Arc::new(report); Some(report) },
                Err(e) => { let _ = tx.send(Event::Log(format!("Rapport périphérique indisponible : {e}"))); None }
            };
            let result = if vendor_running() {
                if let Some(r) = &report { match HidApi::new() { Ok(api) => r.inventory(&api), Err(e) => r.log(&format!("Inventaire impossible : {e}")) } }
                Err(anyhow::anyhow!(
                    "Ferme FIFINE Control Deck, y compris son icône près de l’horloge"
                ))
            } else {
                Deck::open_with_report(report.clone())
            };
            match result {
                Ok(d) => {
                    if let Some(r) = &report { r.log(&format!("Ouverture {} OK ; version firmware non disponible via le protocole connu", d.driver.model().name)); }
                    let model = d.driver.model();
                    let _ = tx.send(Event::DeviceModel {
                        name: model.name.to_owned(), keys: model.keys, rows: model.rows,
                        cols: model.cols, experimental: !model.verified,
                    });
                    deck = Some(d);
                    update = true;
                    beat = Instant::now();
                }
                Err(e) => {
                    if let Some(r) = &report { r.log(&format!("Ouverture périphérique échouée : {e:#} ; code Windows : {:?}", std::io::Error::last_os_error().raw_os_error())); }
                    let _ = tx.send(Event::Device(false, format!("En attente · {e}")));
                }
            }
        }
        let mut failed = None;
        if update || deck.is_none() {
            animations.clear();
            animation_generation = animation_generation.wrapping_add(1);
        }
        if let Some(d) = &deck {
            if update && !test_running {
                match d.apply(&config) {
                    Ok(warnings) => {
                        let _ = tx.send(Event::Device(
                            true,
                            format!("{}{} connecté · {} touches ({} × {}) · rapports {} octets", d.driver.model().name, if d.driver.model().verified { "" } else { " (expérimental)" }, d.driver.model().keys, d.driver.model().rows, d.driver.model().cols, d.packet),
                        ));
                        let _ = tx.send(Event::Applied(revision));
                        if d.driver.model().id == "fifine_d6" && config.d6_animations && config.keys().iter().any(|k| k.image.to_ascii_lowercase().ends_with(".gif")) {
                            let generation = animation_generation;
                            let c = config.clone();
                            let reply = animation_tx.clone();
                            std::thread::spawn(move || {
                                let rendered = crate::key_render::font().ok().map(|font| c.keys().iter()
                                    .map(|key| crate::key_render::animated(key, c.image_size, &font).unwrap_or_default())
                                    .collect::<Vec<_>>()).unwrap_or_default();
                                let _ = reply.send((generation, rendered));
                            });
                        }
                        for warning in warnings {
                            let _ = tx.send(Event::Log(warning));
                        }
                    }
                    Err(e) => failed = Some(e.to_string()),
                }
            }
            if failed.is_none() {
                if let Some(value) = brightness.filter(|_| !test_running) {
                    if let Err(e) = d.set_brightness(value) {
                        failed = Some(e.to_string());
                    }
                }
            }
            while let Ok((generation, rendered)) = animation_rx.try_recv() {
                if generation == animation_generation {
                    animations = rendered;
                    next_frame = [1; 15];
                    due = [Instant::now(); 15];
                }
            }
            if failed.is_none() && test_running {
                if let Some(at) = test_brightness_at {
                    if at.elapsed() >= Duration::from_millis(900) {
                        match d.set_brightness(100) {
                            Ok(()) => { let _=tx.send(Event::HardwareTestStatus("Luminosité 30 % / 100 % : commandes OK (effet visuel à confirmer)".into())); },
                            Err(e) => failed=Some(format!("Test luminosité 100 % : {e}")),
                        }
                        test_brightness_at=None;
                    }
                }
            }
            if failed.is_none() && beat.elapsed() >= Duration::from_secs(8) {
                if vendor_running() {
                    failed = Some(
                        "FIFINE a été relancé : connexion suspendue pour éviter les conflits"
                            .into(),
                    );
                } else if d.driver.model().family == Family::Mirabox { if let Err(e) = d.keepalive() { failed = Some(e.to_string()); }
                }
                beat = Instant::now();
            }
            if failed.is_none() {
                let mut data = [0u8; 2048];
                match d.hid.read_timeout(&mut data, 30) {
                    Ok(n) if n > 0 => {
                        if let Some(r) = &d.report { if d.inputs.get() < 20 { r.log(&format!("Entrée HID #{}{} : {}", d.inputs.get() + 1, if data.starts_with(b"ACK") || data.starts_with(b"\0ACK") { " (réponse/ACK)" } else { "" }, report::hex(&data[..n]))); d.inputs.set(d.inputs.get() + 1); } }
                        if d.driver.model().id == "elgato_plus" && data.get(1).is_some_and(|tag| *tag == 2 || *tag == 3) {
                            d.note(&format!("Stream Deck Plus : écran/molette ignoré (type {})", data[1]));
                        }
                        if test_running && !data.starts_with(b"ACK") && !data.starts_with(b"\0ACK") { d.note(&format!("Test entrée brute : {}", report::hex(&data[..n]))); }
                        for event in d.decode_input(&data[..n]) {
                            match event {
                                InputEvent::Key(mut k, down) if k < d.driver.model().keys as usize && k < config.keys().len() => {
                                    if config.flip_input_rows && d.driver.model().id == "fifine_d6" { k = display_index(k); }
                                    if down && test_running {
                                        d.note(&format!("Test appui : brut={} ; index décodé={}", report::hex(&data[..n]), k+1));
                                        let _=tx.send(Event::HardwareTestPress(report::hex(&data[..n]), k));
                                    } else if test_running { /* Relâchement ignoré pendant le test. */
                                    } else if down {
                                        if last_press[k].elapsed() >= Duration::from_millis(150) {
                                            last_press[k] = Instant::now();
                                            let _ = tx.send(Event::Key(k, Instant::now()));
                                        }
                                    } else { let _ = tx.send(Event::KeyRelease(k, Instant::now())); }
                                }
                                InputEvent::Encoder(i, direction) => d.note(&format!("Molette {i} : {direction:+} (action non configurée)")),
                                _ => {}
                            }
                        }
                    }
                    Err(e) => failed = Some(e.to_string()),
                    _ => {}
                }
            }
            // Pending runtime faces are replayed after a complete apply, never interleaved.
            if failed.is_none() && !update && !test_running {
                if let Some(index) = key_images.iter().position(Option::is_some) {
                    if let Some(jpeg) = key_images[index].take() {
                        if let Err(e)=d.set_key_image(index,&jpeg,frame_end()) { failed=Some(e.to_string()); }
                    }
                }
            }
            // Read HID before every animation frame; uploads never preempt a full apply.
            let now = Instant::now();
            let lengths = std::array::from_fn(|i| animations.get(i).map_or(0, Vec::len));
            if let Some(index) = next_animation_frame(now, update || failed.is_some(),
                config.d6_animations && !test_running, config.animation_fps, &lengths, &due,
                &last_per_key, last_frame, cursor) {
                let frames = &animations[index];
                let frame = &frames[next_frame[index] % frames.len()];
                match d.set_key_image(index, &frame.jpeg, frame_end()) {
                    Ok(()) => {
                        let sent = Instant::now();
                        due[index] = sent + frame.delay.max(Duration::from_millis(1000 / config.animation_fps.max(1) as u64));
                        last_per_key[index] = sent;
                        next_frame[index] += 1;
                        cursor = (index + 1) % 15;
                        last_frame = sent;
                    }
                    Err(e) => failed = Some(e.to_string()),
                }
            }
        } else {
            std::thread::sleep(Duration::from_millis(50));
        }
        if let Some(error) = failed {
            if test_running { let _=tx.send(Event::HardwareTestStatus(format!("ÉCHEC périphérique : {error}"))); test_running=false; test_brightness_at=None; }
            if let Some(d) = &deck { d.note(&format!("Connexion interrompue : {error}")); }
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
    decode_input(data).and_then(|(index, down)| down.then_some(index))
}
pub fn decode_input(data: &[u8]) -> Option<(usize, bool)> {
    let d = if data.first() == Some(&0) {
        &data[1..]
    } else {
        data
    };
    if d.len() < 11
        || &d[..3] != b"ACK"
        || &d[5..7] != b"OK"
        || !(1..=15).contains(&d[9])
        || !matches!(d[10], 0 | 1)
    {
        return None;
    }
    Some((d[9] as usize - 1, d[10] == 1))
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
    report: Option<Arc<Report>>,
    inputs: Cell<u8>,
    driver: Driver,
    // For non-D6 Mirabox models the same tag can represent alternating edges.
    transitions: Cell<u64>,
}
#[derive(Clone, Copy)]
enum Driver { Mirabox(&'static DeviceModel) }
impl Driver {
    fn model(self) -> &'static DeviceModel { match self { Self::Mirabox(model) => model } }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputEvent { Key(usize, bool), Encoder(usize, i8) }

// Pure transformations and packet construction: no HID handle and no hardware in tests.
fn mirabox_key(model: &DeviceModel, index: usize) -> Result<u8> {
    anyhow::ensure!(index < model.keys as usize && index < 15, "Touche hors grille : {index}");
    let index = if model.id == "fifine_d6" { display_index(index) } else { index };
    Ok(index as u8 + 1)
}
fn bat_tail(model: &DeviceModel, index: usize, len: usize) -> Result<[u8; 8]> {
    anyhow::ensure!(len <= 65535, "Image trop lourde");
    Ok([b'B', b'A', b'T', 0, 0, (len >> 8) as u8, len as u8, mirabox_key(model, index)?])
}
fn canonical_image(jpeg: &[u8], model: &DeviceModel) -> Result<Vec<u8>> {
    if model.id == "fifine_d6" { return Ok(jpeg.to_vec()); }
    // key_render produces D6-oriented pixels (180 degrees). Undo that transform,
    // then apply the catalogue orientation; no re-encoding on the D6 path.
    let source = image::load_from_memory(jpeg)?.to_rgb8();
    let source = image::imageops::rotate180(&source);
    let mut out = image::imageops::resize(&source, model.image_w.into(), model.image_h.into(), image::imageops::FilterType::Lanczos3);
    out = match model.rotation {
        0 => out, 90 => image::imageops::rotate90(&out),
        180 => image::imageops::rotate180(&out),
        270 => image::imageops::rotate270(&out),
        _ => anyhow::bail!("Rotation non prise en charge"),
    };
    if model.mirror_x { out = image::imageops::flip_horizontal(&out); }
    if model.mirror_y { out = image::imageops::flip_vertical(&out); }
    let mut bytes = Vec::new();
    match model.image_format {
        ImageFormat::Jpeg => image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 88)
            .encode(&out, out.width(), out.height(), image::ExtendedColorType::Rgb8)?,
        ImageFormat::Bmp => image::codecs::bmp::BmpEncoder::new(&mut bytes)
            .encode(&out, out.width(), out.height(), image::ExtendedColorType::Rgb8)?,
    }
    Ok(bytes)
}
fn decode_model_input(model: &DeviceModel, data: &[u8], transitions: &Cell<u64>) -> Vec<InputEvent> {
    if model.id == "fifine_d6" {
        return decode_input(data).map(|(k, down)| vec![InputEvent::Key(k, down)]).unwrap_or_default();
    }
    if model.family == Family::Elgato {
        if let Some(states) = elgato::keys(model, data) {
            let mut events = Vec::new();
            for (index, down) in states {
                let mask = 1u64 << index;
                if (transitions.get() & mask != 0) != down {
                    transitions.set(transitions.get() ^ mask);
                    events.push(InputEvent::Key(index, down));
                }
            }
            return events;
        }
        return Vec::new();
    }
    let d = if data.starts_with(b"\0ACK") { &data[1..] } else { data };
    if d.len() < 11 || !(d.starts_with(b"ACK\0\0OK") || d[..9].iter().all(|&byte| byte == 0)) { return Vec::new(); }
    let tag = d[9];
    if (1..=model.keys).contains(&tag) {
        let mask = 1u64 << (tag - 1);
        let down = transitions.get() & mask == 0;
        transitions.set(transitions.get() ^ mask);
        return vec![InputEvent::Key(tag as usize - 1, down)];
    }
    let encoder = match tag {
        0x90 => Some((0, -1)), 0x91 => Some((0, 1)),
        0x50 => Some((1, -1)), 0x51 => Some((1, 1)),
        0x60 => Some((2, -1)), 0x61 => Some((2, 1)),
        _ => None,
    };
    encoder.filter(|(index, _)| *index < model.encoders as usize)
        .map(|(index, direction)| vec![InputEvent::Encoder(index, direction)]).unwrap_or_default()
}

// Hardware hypothesis: a single BAT upload ends with STP, without CLE.
// Change this constant to UlendStp if the D6 firmware requires the full terminator.
#[derive(Clone, Copy)]
enum FrameEnd { Stp, UlendStp }
fn frame_end() -> FrameEnd {
    if std::env::var_os("DECKFORGE_D6_FRAME_ULEND").is_some() { FrameEnd::UlendStp } else { FrameEnd::Stp }
}
impl Deck {
    fn send_frame(&self, idx: usize, jpeg: &[u8], end: FrameEnd) -> Result<()> {
        let started = Instant::now();
        let data = canonical_image(jpeg, self.driver.model())?;
        if self.driver.model().family == Family::Elgato {
            for page in elgato::image_reports(self.driver.model(), idx, &data)? { self.report(&page)?; }
            self.note(&format!("Image Elgato touche {} : {} octets", idx + 1, data.len()));
            return Ok(());
        }
        self.command(&bat_tail(self.driver.model(), idx, data.len())?)?;
        for chunk in data.chunks(self.packet) { self.report(chunk)?; }
        self.await_ack()?;
        if matches!(end, FrameEnd::UlendStp) { self.command(b"ULEND")?; }
        let result = self.command(b"STP");
        self.note(&format!("Image touche {} : {} octets en {} ms ; résultat {:?}", idx + 1, jpeg.len(), started.elapsed().as_millis(), result.as_ref().err().map(ToString::to_string)));
        result
    }
    fn set_key_image(&self, idx: usize, image: &[u8], end: FrameEnd) -> Result<()> { self.send_frame(idx, image, end) }
    fn set_brightness(&self, value: u8) -> Result<()> {
        if self.driver.model().family == Family::Elgato { self.feature(&elgato::brightness(self.driver.model(), value)) }
        else { self.command(&[b'L', b'I', b'G', 0, 0, value.min(100)]) }
    }
    fn clear(&self) -> Result<()> { if self.driver.model().family == Family::Elgato { self.feature(&elgato::reset(self.driver.model())) } else { self.command(&[b'C', b'L', b'E', 0, 0, 0, 0xFF]) } }
    fn feature(&self, bytes: &[u8]) -> Result<()> { self.hid.send_feature_report(bytes).context("Rapport de fonctionnalité Elgato refusé") }
    fn sleep(&self) -> Result<()> { self.report(b"HAN") }
    fn wake(&self) -> Result<()> { self.command(b"DIS") }
    fn keepalive(&self) -> Result<()> { self.command(b"CONNECT") }
    fn decode_input(&self, data: &[u8]) -> Vec<InputEvent> { decode_model_input(self.driver.model(), data, &self.transitions) }
    fn await_ack(&self) -> Result<()> {
        if self.driver.model().id == "fifine_d6" { return Ok(()); } // Never change D6 timing.
        let mut data = [0u8; 2048];
        for _ in 0..4 {
            let n = self.hid.read_timeout(&mut data, 50)?;
            if n == 0 { continue; }
            if data[..n].starts_with(b"ACK\0\0OK") || data[..n].starts_with(b"\0ACK\0\0OK") {
                self.note("ACK image reçu"); return Ok(());
            }
            self.note(&format!("Entrée pendant attente ACK : {}", report::hex(&data[..n.min(32)])));
        }
        anyhow::bail!("ACK image absent après 200 ms pour {}", self.driver.model().name)
    }
    fn open() -> Result<Self> { Self::open_with_report(None) }
    fn open_with_report(report: Option<Arc<Report>>) -> Result<Self> {
        let api = HidApi::new()?;
        if let Some(r) = &report { r.inventory(&api); }
        let infos: Vec<_> = api.device_list().collect();
        let candidates: Vec<_> = infos.iter().map(|d| devices::HidInfo {
            vid: d.vendor_id(), pid: d.product_id(), usage_page: d.usage_page(),
            usage: d.usage(), interface: d.interface_number(),
            product: d.product_string().map(str::to_owned),
            manufacturer: d.manufacturer_string().map(str::to_owned),
            path: d.path().to_string_lossy().into_owned(),
        }).collect();
        let mut matches: Vec<_> = devices::detect(&candidates).into_iter().filter_map(|item| {
            if let Detection::Matched { info, model } = item { Some((info, model)) } else { None }
        }).collect();
        // Verified devices take priority regardless of HID enumeration order.
        matches.sort_by_key(|(_, model)| (
            !model.verified,
            !(model.family == Family::Mirabox && matches!(model.protocol, Protocol::MiraboxV1 | Protocol::MiraboxV2)),
        ));
        if let Some(r) = &report {
            for (info, model) in matches.iter().skip(1) {
                r.log(&format!("Autre périphérique compatible ignoré : {} {:04X}:{:04X}{}", model.name, info.vid, info.pid, if model.verified { "" } else { " (expérimental)" }));
            }
        }
        let (chosen, model) = matches.first().context("Aucun périphérique compatible détecté")?;
        anyhow::ensure!(model.family == Family::Elgato || model.family == Family::Mirabox && matches!(model.protocol, Protocol::MiraboxV1 | Protocol::MiraboxV2),
            "{} reconnu (expérimental) : pilote de ce protocole non disponible", model.name);
        let info = infos.iter().find(|d| d.path().to_string_lossy() == chosen.path)
            .context("Interface HID disparue pendant la détection")?;
        if let Some(r) = &report { r.log(&format!("Pilote sélectionné : {}{} ; {} touches {}×{} ; protocole {:?}", model.name, if model.verified { "" } else { " expérimental, non vérifié sur matériel" }, model.keys, model.rows, model.cols, model.protocol)); }
        let hid = info.open_device(&api).map_err(|e| {
            if let Some(r) = &report { r.log(&format!("Ouverture HID refusée : {e} ; code Windows {:?}", std::io::Error::last_os_error().raw_os_error())); }
            e
        }).context("Accès au périphérique impossible")?;
        let mut descriptor = [0u8; 4096];
        let n = hid.get_report_descriptor(&mut descriptor)?;
        let (packet, rid) = if model.family == Family::Elgato { (model.packet_len as usize, 0) } else { output_report(&descriptor[..n])? };
        if let Some(r) = &report { r.log(&format!("Rapport sortie={} octets, ID={rid} ; tampon entrée=2048 octets ; descripteur={} octets", packet, n)); }
        if model.id != "fifine_d6" { anyhow::ensure!(packet == model.packet_len as usize,
            "{} : rapport HID {} octets, {} attendus ; abandon prudent", model.name, packet, model.packet_len); }
        if model.family == Family::Elgato {
            let (id, len, _) = elgato::firmware_layout(model);
            let mut buf = vec![0; len]; buf[0] = id;
            match hid.get_feature_report(&mut buf) {
                Ok(n) => if let Some(r) = &report { r.log(&format!("Firmware Elgato : {}", elgato::firmware(model, &buf[..n]).unwrap_or_else(|| "réponse non décodable".into()))); },
                Err(e) => if let Some(r) = &report { r.log(&format!("Firmware Elgato indisponible : {e}")); },
            }
        }
        Ok(Self { hid, packet, rid, report, inputs: Cell::new(0), driver: Driver::Mirabox(model), transitions: Cell::new(0) })
    }
    fn note(&self, line: &str) { if let Some(r) = &self.report { r.log(line); } }
    fn report(&self, payload: &[u8]) -> Result<()> {
        anyhow::ensure!(payload.len() <= self.packet, "Rapport trop long");
        let mut data = vec![0u8; self.packet + 1];
        data[0] = self.rid;
        data[1..1 + payload.len()].copy_from_slice(payload);
        // windows-native reports 0 for a synchronously completed WriteFile.
        let n = self.hid.write(&data).map_err(|e| { self.note(&format!("Erreur écriture HID : {e} ; code Windows {:?}", std::io::Error::last_os_error().raw_os_error())); e })?;
        anyhow::ensure!(
            n == 0 || n == data.len(),
            "Rapport HID partiellement envoyé"
        );
        Ok(())
    }
    fn command(&self, tail: &[u8]) -> Result<()> {
        let mut data = b"CRT\0\0".to_vec();
        data.extend(tail);
        self.note(&format!("Commande {} : {}", String::from_utf8_lossy(&tail[..tail.len().min(3)]), report::hex(&data[..data.len().min(16)])));
        self.report(&data)
    }
    fn apply(&self, c: &Config) -> Result<Vec<String>> {
        c.validate()?;
        // Prepare all images first; one broken file falls back to its built-in icon.
        let font = crate::key_render::font()?;
        let rendered = c
            .keys()
            .iter()
            .take(self.driver.model().keys as usize)
            .map(|k| render_key_or_default(k, c.image_size, &font))
            .collect::<Result<Vec<_>>>()?;
        let (images, warnings): (Vec<_>, Vec<_>) = rendered.into_iter().unzip();
        if self.driver.model().family == Family::Elgato {
            self.set_brightness(c.brightness)?;
            for (idx, jpeg) in images.iter().enumerate() { self.send_frame(idx, jpeg, FrameEnd::Stp)?; }
            return Ok(warnings.into_iter().flatten().collect());
        }
        self.wake()?;
        self.set_brightness(c.brightness)?;
        self.note("Commande HAN : 48 41 4E");
        self.sleep()?;
        self.clear()?;
        for (idx, jpeg) in images.iter().enumerate() {
            let data = canonical_image(jpeg, self.driver.model())?;
            self.command(&bat_tail(self.driver.model(), idx, data.len())?)?;
            let started = Instant::now();
            for chunk in data.chunks(self.packet) { self.report(chunk)?; }
            self.await_ack()?;
            self.note(&format!("Image initialisation touche {} : {} octets en {} ms", idx + 1, jpeg.len(), started.elapsed().as_millis()));
        }
        self.command(b"ULEND")?;
        self.command(b"STP")?;
        Ok(warnings.into_iter().flatten().collect())
    }
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
    fn audio_states_produce_distinct_lcd_jpegs_even_with_an_old_static_image() {
        let mut key = crate::audio::tests::key();
        if let crate::model::Step::AudioSwitch {
            first_label,
            second_label,
            ..
        } = &mut key.steps[0]
        {
            *first_label = "Audio".into();
            *second_label = "Audio".into();
        }
        let windir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
        let font = FontArc::try_from_vec(
            std::fs::read(std::path::PathBuf::from(windir).join("Fonts/segoeui.ttf")).unwrap(),
        )
        .unwrap();
        let a = crate::audio::present_key(
            &key,
            &crate::audio::Snapshot {
                output: "headset".into(),
                ..Default::default()
            },
        );
        let b = crate::audio::present_key(
            &key,
            &crate::audio::Snapshot {
                output: "speakers".into(),
                ..Default::default()
            },
        );
        let a = render_key(&a, 126, &font).unwrap();
        let b = render_key(&b, 126, &font).unwrap();
        assert_ne!(a, b);
        assert_eq!(image::load_from_memory(&a).unwrap().width(), 126);
        let dir = tempfile::tempdir().unwrap();
        let svg = dir.path().join("custom.svg");
        std::fs::write(&svg,br##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="#00ff00"/></svg>"##).unwrap();
        if let crate::model::Step::AudioSwitch { first_icon, .. } = &mut key.steps[0] {
            *first_icon = svg.to_string_lossy().into_owned();
        }
        let custom = crate::audio::present_key(
            &key,
            &crate::audio::Snapshot {
                output: "headset".into(),
                ..Default::default()
            },
        );
        let bitmap = image::load_from_memory(&render_key(&custom, 126, &font).unwrap())
            .unwrap()
            .to_rgb8();
        let pixel = bitmap.get_pixel(63, 63);
        assert!(pixel[1] > 200 && pixel[0] < 30);
    }
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
        assert_eq!(decode_input(&data), None);
        // Captured from this Windows D6: release is 0, not 2.
        data[10] = 0;
        assert_eq!(decode_press(&data), None);
        assert_eq!(decode_input(&data), Some((14, false)));
        data[9] = 16;
        assert_eq!(decode_press(&data), None);
        assert_eq!(decode_press(&[0]), None);
    }
    #[test]
    fn generic_mirabox_packets_preserve_legacy_d6_bytes() {
        let d6 = &devices::MODELS[0];
        for idx in 0..15 {
            for len in [0, 1, 511, 512, 65535] {
                let old = [b'B', b'A', b'T', 0, 0, (len >> 8) as u8, len as u8, display_index(idx) as u8 + 1];
                assert_eq!(bat_tail(d6, idx, len).unwrap(), old);
            }
        }
        assert!(bat_tail(d6, 15, 1).is_err());
        assert!(bat_tail(d6, 0, 65536).is_err());
        let other = devices::MODELS.iter().find(|m| m.id == "akp03").unwrap();
        assert_eq!(bat_tail(other, 0, 0x1234).unwrap(), [b'B', b'A', b'T', 0, 0, 0x12, 0x34, 1]);
        assert!(bat_tail(other, 6, 1).is_err());
        let jpeg = crate::key_render::jpeg(&Config::default().keys()[0], 126, &crate::key_render::font().unwrap()).unwrap();
        assert_eq!(canonical_image(&jpeg, d6).unwrap(), jpeg);
        let converted = canonical_image(&jpeg, other).unwrap();
        let img = image::load_from_memory(&converted).unwrap();
        assert_eq!((img.width(), img.height()), (60, 60));
    }
    #[test]
    fn input_events_are_pure_and_d6_release_is_unchanged() {
        let d6 = &devices::MODELS[0];
        let mut data = [0u8; 12]; data[..7].copy_from_slice(b"ACK\0\0OK"); data[9] = 1; data[10] = 1;
        let state = Cell::new(0);
        assert_eq!(decode_model_input(d6, &data, &state), vec![InputEvent::Key(0, true)]);
        data[10] = 0;
        assert_eq!(decode_model_input(d6, &data, &state), vec![InputEvent::Key(0, false)]);
        let other = devices::MODELS.iter().find(|m| m.id == "akp03").unwrap();
        assert_eq!(decode_model_input(other, &data, &state), vec![InputEvent::Key(0, true)]);
        assert_eq!(decode_model_input(other, &data, &state), vec![InputEvent::Key(0, false)]);
        data[..9].fill(0); // Some Mirabox models report raw zero-prefixed key transitions.
        assert_eq!(decode_model_input(other, &data, &state), vec![InputEvent::Key(0, true)]);
        data[9] = 0x91;
        assert_eq!(decode_model_input(other, &data, &state), vec![InputEvent::Encoder(0, 1)]);
        data[9] = 0xff;
        assert!(decode_model_input(other, &data, &state).is_empty());
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

#[cfg(test)]
mod animation_scheduler_tests {
    use super::*;
    #[test]
    fn rate_limits_fairness_and_apply_barrier() {
        let now = Instant::now();
        let mut due = [now; 15];
        let mut per_key = [now - Duration::from_secs(1); 15];
        let mut lengths = [0; 15];
        lengths[0] = 3; lengths[1] = 2;
        let old = now - Duration::from_secs(1);
        let choose = |time, apply, enabled, due: &[Instant;15], per: &[Instant;15], global, cursor|
            next_animation_frame(time, apply, enabled, 12, &lengths, due, per, global, cursor);
        assert_eq!(choose(now, false, true, &due, &per_key, old, 0), Some(0));
        per_key[0] = now; due[0] = now + Duration::from_millis(84);
        assert_eq!(choose(now + Duration::from_millis(33), false, true, &due, &per_key, now, 1), None);
        assert_eq!(choose(now + Duration::from_millis(34), false, true, &due, &per_key, now, 1), Some(1));
        assert_eq!(choose(now + Duration::from_millis(84), false, true, &due, &per_key, now, 0), Some(0));
        assert_eq!(choose(now + Duration::from_millis(84), true, true, &due, &per_key, old, 0), None);
        assert_eq!(choose(now + Duration::from_millis(84), false, false, &due, &per_key, old, 0), None);
        due[1] = now; per_key[1] = now;
        assert_eq!(choose(now + Duration::from_millis(84), false, true, &due, &per_key, old, 1), Some(1));
    }
}
