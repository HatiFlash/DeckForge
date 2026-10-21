//! Opt-in, local and bounded UI trace. Only fixed-category and numerical signals are accepted.
use serde_json::{json, Value};
use std::{fs::{self, File}, io::{BufWriter, Write}, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}, mpsc::{self, SyncSender, TrySendError}}, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};
const LIMIT: u64 = 6 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(90);
const QUEUE: usize = 256;

pub struct Session {
    pub path: PathBuf,
    pub id: u128,
    started: Instant,
    active: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
    sender: SyncSender<Value>,
    last_sample: Instant,
    last_zone: &'static str,
    last_frame: Instant,
    frame: u64,
}
fn timestamp() -> u128 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() }
fn write_event(writer: &mut BufWriter<File>, bytes: &mut u64, value: &Value) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    if *bytes + line.len() as u64 > LIMIT { return Ok(()); }
    writer.write_all(&line)?;
    *bytes += line.len() as u64;
    Ok(())
}
fn open(dir: &Path, id: u128) -> std::io::Result<(PathBuf, File)> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("flicker-{id}.jsonl"));
    let file = fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
    Ok((path, file))
}
impl Session {
    pub fn start(config: &Path) -> std::io::Result<Self> {
        let id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        let primary = config.parent().unwrap_or_else(|| Path::new(".")).join("diagnostics");
        let (path, file) = open(&primary, id).or_else(|_| {
            let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            open(&crate::assets::user_data_dir(&local).join("diagnostics"), id)
        })?;
        let (sender, receiver) = mpsc::sync_channel::<Value>(QUEUE);
        let active = Arc::new(AtomicBool::new(true));
        let dropped = Arc::new(AtomicU64::new(0));
        let worker_active = active.clone();
        let worker_dropped = dropped.clone();
        std::thread::spawn(move || {
            let start = Instant::now();
            let mut writer = BufWriter::new(file);
            let mut bytes = 0u64;
            let _ = write_event(&mut writer, &mut bytes, &json!({"kind":"start","session_id":id,"timestamp_ms":timestamp(),"monotonic_ms":0,"frame_id":0}));
            while worker_active.load(Ordering::Relaxed) && start.elapsed() < TIMEOUT && bytes < LIMIT.saturating_sub(1024) {
                match receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(event) => { if write_event(&mut writer, &mut bytes, &event).is_err() { break; } },
                    Err(mpsc::RecvTimeoutError::Timeout) => {},
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            worker_active.store(false, Ordering::Relaxed);
            // Drain a bounded queue before closing; no producer writes after active=false.
            while let Ok(event) = receiver.try_recv() { if write_event(&mut writer, &mut bytes, &event).is_err() { break; } }
            let written = bytes;
            let _ = write_event(&mut writer, &mut bytes, &json!({"kind":"stop","session_id":id,"timestamp_ms":timestamp(),"monotonic_ms":start.elapsed().as_millis(),"frame_id":0,"dropped":worker_dropped.load(Ordering::Relaxed),"bytes":written}));
            let _ = writer.flush();
        });
        let now = Instant::now();
        Ok(Self { path, id, started: now, active, dropped, sender, last_sample: now - Duration::from_millis(100), last_zone: "outside", last_frame: now, frame: 0 })
    }
    pub fn active(&self) -> bool { self.active.load(Ordering::Relaxed) && self.started.elapsed() < TIMEOUT }
    pub fn stop(&self) { self.active.store(false, Ordering::Relaxed); }
    pub fn dropped(&self) -> u64 { self.dropped.load(Ordering::Relaxed) }
    pub fn record(&mut self, mut event: Value) {
        if !self.active() { return; }
        event["session_id"] = json!(self.id);
        event["timestamp_ms"] = json!(timestamp());
        event["monotonic_ms"] = json!(self.started.elapsed().as_millis());
        event["frame_id"] = json!(self.frame);
        if let Err(TrySendError::Full(_)) = self.sender.try_send(event) { self.dropped.fetch_add(1, Ordering::Relaxed); }
    }
    pub fn frame(&mut self, zone: &'static str, details: Value, elapsed_ms: f64) {
        self.frame += 1;
        if !self.active() { return; }
        let interval = self.last_frame.elapsed().as_secs_f64() * 1000.0;
        self.last_frame = Instant::now();
        let changed = zone != self.last_zone;
        self.last_zone = zone;
        if changed || elapsed_ms > 35.0 || interval > 100.0 || self.last_sample.elapsed() >= Duration::from_millis(75) {
            self.last_sample = Instant::now();
            self.record(json!({"kind":"frame","zone":zone,"zone_changed":changed,"update_ms":elapsed_ms,"frame_interval_ms":interval,"metrics":details,"dropped":self.dropped()}));
        }
    }
}
impl Drop for Session { fn drop(&mut self) { self.stop(); } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_jsonl_stop_bounds_and_no_data_when_disabled() {
        let root = std::env::temp_dir().join(format!("deckforge-flicker-test-{}", timestamp()));
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config.json");
        let mut session = Session::start(&config).unwrap();
        let path = session.path.clone();
        session.frame("preview", json!({"mouse_x":18,"width":500}), 4.0);
        session.frame("inspector", json!({"mouse_y":240,"width":300}), 42.0);
        for _ in 0..300 { session.record(json!({"kind":"test","number":7})); }
        session.stop();
        let deadline = Instant::now() + Duration::from_secs(4);
        let text = loop {
            let text = fs::read_to_string(&path).unwrap();
            if text.lines().any(|line| line.contains("\"kind\":\"stop\"")) { break text; }
            assert!(Instant::now() < deadline, "writer did not flush on stop");
            std::thread::sleep(Duration::from_millis(20));
        };
        let rows: Vec<Value> = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert!(rows.iter().any(|v| v["kind"] == "frame" && v["zone"] == "preview"));
        assert!(rows.iter().any(|v| v["kind"] == "frame" && v["zone"] == "inspector"));
        assert_eq!(rows.last().unwrap()["kind"], "stop");
        assert!(text.len() < LIMIT as usize);
        assert!(!text.contains("config.json"));
        let before = fs::metadata(&path).unwrap().len();
        session.record(json!({"kind":"test"}));
        assert_eq!(before, fs::metadata(&path).unwrap().len());
        drop(session);
        fs::remove_dir_all(root).unwrap();
    }
}
