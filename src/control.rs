/// State for long actions. Releases may be absent; the deadline always completes the hold.
#[derive(Default, Debug)]
pub struct LongPress {
    pending: [Option<std::time::Instant>; 15],
    held: [bool; 15],
}
impl LongPress {
    pub fn clear(&mut self) { self.pending.fill(None); self.held.fill(false); }
    /// True means execute the ordinary binding immediately.
    pub fn press(&mut self, index: usize, at: std::time::Instant, has_long: bool) -> bool {
        if !has_long { self.pending[index] = None; self.held[index] = false; return true; }
        if self.held[index] { return false; }
        self.held[index] = true;
        self.pending[index] = Some(at);
        false
    }
    /// Return whether the released binding is long; None for an unmatched release.
    pub fn release(&mut self, index: usize, at: std::time::Instant, threshold_ms: u64) -> Option<bool> {
        self.held[index] = false;
        self.pending[index].take().map(|start| at.saturating_duration_since(start) >= std::time::Duration::from_millis(threshold_ms))
    }
    pub fn due(&mut self, now: std::time::Instant, threshold_ms: u64) -> Vec<usize> {
        let threshold = std::time::Duration::from_millis(threshold_ms);
        let mut ready = Vec::new();
        for (index, pending) in self.pending.iter_mut().enumerate() {
            if pending.is_some_and(|start| now.saturating_duration_since(start) >= threshold) {
                *pending = None;
                ready.push(index);
            }
        }
        ready
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Execute,
    Inspect,
    Blocked(&'static str),
}

#[derive(Debug)]
pub struct InputGate {
    pub enabled: bool,
    pub inspect: bool,
    pub connected: bool,
    pub dirty: bool,
    pub invalid: bool,
    revision: u64,
    applied: Option<u64>,
}
impl Default for InputGate {
    fn default() -> Self {
        Self {
            enabled: true,
            inspect: false,
            connected: false,
            dirty: false,
            invalid: false,
            revision: 0,
            applied: None,
        }
    }
}
impl InputGate {
    pub fn request_apply(&mut self) -> u64 {
        self.revision += 1;
        self.applied = None;
        self.dirty = false;
        self.revision
    }
    pub fn applied(&mut self, revision: u64) -> bool {
        if revision != self.revision {
            return false;
        }
        self.applied = Some(revision);
        true
    }
    pub fn disconnect(&mut self) {
        self.connected = false;
        self.applied = None;
    }
    pub fn pending(&self) -> bool {
        self.applied != Some(self.revision)
    }
    pub fn decision_for_age(&self, age: std::time::Duration) -> Decision {
        if age > std::time::Duration::from_millis(500) {
            return Decision::Blocked("Appui ancien ignoré");
        }
        self.decision()
    }
    pub fn decision(&self) -> Decision {
        if !self.connected {
            return Decision::Blocked("D6 déconnecté");
        }
        if self.invalid {
            return Decision::Blocked("Configuration à corriger");
        }
        if self.inspect {
            return Decision::Inspect;
        }
        if !self.enabled {
            return Decision::Blocked("Touches en pause");
        }
        if self.dirty {
            return Decision::Blocked("Enregistre tes modifications pour activer les touches");
        }
        if self.pending() {
            return Decision::Blocked("Synchronisation du D6 en cours");
        }
        Decision::Execute
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_press_deadline_release_and_missing_release() {
        use std::time::{Duration, Instant};
        let t = Instant::now();
        let mut state = LongPress::default();
        assert!(state.press(0, t, false));
        assert_eq!(state.release(0, t + Duration::from_millis(10), 500), None);
        assert!(!state.press(1, t, true));
        assert!(state.due(t + Duration::from_millis(499), 500).is_empty());
        assert_eq!(state.release(1, t + Duration::from_millis(499), 500), Some(false));
        assert!(!state.press(1, t, true));
        assert_eq!(state.release(1, t + Duration::from_millis(500), 500), Some(true));
        assert!(!state.press(2, t, true));
        assert_eq!(state.due(t + Duration::from_millis(500), 500), vec![2]);
        assert!(!state.press(2, t + Duration::from_millis(600), true));
        assert!(state.due(t + Duration::from_millis(1200), 500).is_empty());
        assert_eq!(state.release(2, t + Duration::from_millis(800), 500), None);
        assert!(!state.press(2, t + Duration::from_millis(900), true));
        assert_eq!(state.due(t + Duration::from_millis(1400), 500), vec![2]);
        state.press(3, t, true);
        state.clear();
        assert!(state.due(t + Duration::from_secs(1), 500).is_empty());
    }
    #[test]
    fn captured_usb_presses_reach_macro_engine_once_each() {
        use std::sync::{atomic::Ordering, mpsc};
        use std::time::{Duration, Instant};
        let capture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/d6-input.json")).unwrap();
        let (tx, rx) = mpsc::channel();
        let engine = crate::engine::Engine::new(tx);
        let mut gate = InputGate::default();
        let revision = gate.request_apply();
        gate.connected = true;
        gate.applied(revision);
        let mut keys = vec![];
        for raw in capture["packets"].as_array().unwrap() {
            let bytes = raw
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_u64().unwrap() as u8)
                .collect::<Vec<_>>();
            if let Some(index) = crate::device::decode_press(&bytes) {
                assert_eq!(gate.decision(), Decision::Execute);
                keys.push(index + 1);
                engine.start(
                    crate::model::Key {
                        label: format!("Test {}", index + 1),
                        steps: vec![crate::model::Step::Delay { ms: 1 }],
                        ..Default::default()
                    },
                    false,
                    0,
                );
                let end = Instant::now() + Duration::from_secs(1);
                while engine.running.load(Ordering::SeqCst) && Instant::now() < end {
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert!(!engine.running.load(Ordering::SeqCst));
            }
        }
        assert_eq!(keys, vec![1, 8, 15]);
        assert_eq!(
            rx.try_iter()
                .filter(|e| matches!(e,crate::Event::Log(s)if s.starts_with("EXÉCUTION")))
                .count(),
            3
        );
    }
    #[test]
    fn initial_connection_activates_physical_keys_without_hidden_checkboxes() {
        let mut gate = InputGate::default();
        let r = gate.request_apply();
        gate.connected = true;
        assert!(matches!(gate.decision(), Decision::Blocked(_)));
        assert!(gate.applied(r));
        assert_eq!(gate.decision(), Decision::Execute);
    }
    #[test]
    fn old_usb_ack_cannot_enable_new_configuration() {
        let mut gate = InputGate::default();
        gate.connected = true;
        let old = gate.request_apply();
        let new = gate.request_apply();
        assert!(!gate.applied(old));
        assert!(matches!(gate.decision(), Decision::Blocked(_)));
        gate.applied(new);
        assert_eq!(gate.decision(), Decision::Execute);
    }
    #[test]
    fn editing_during_upload_stays_blocked_until_latest_save() {
        let mut gate = InputGate::default();
        gate.connected = true;
        let r = gate.request_apply();
        gate.dirty = true;
        gate.applied(r);
        assert!(matches!(gate.decision(), Decision::Blocked(_)));
        let r = gate.request_apply();
        gate.applied(r);
        assert_eq!(gate.decision(), Decision::Execute);
        gate.enabled = false;
        assert_eq!(gate.decision(), Decision::Blocked("Touches en pause"));
    }
    #[test]
    fn diagnostic_never_executes_and_reconnect_requires_upload() {
        let mut gate = InputGate::default();
        let r = gate.request_apply();
        gate.connected = true;
        gate.applied(r);
        gate.inspect = true;
        assert_eq!(gate.decision(), Decision::Inspect);
        gate.disconnect();
        gate.connected = true;
        gate.inspect = false;
        assert!(matches!(gate.decision(), Decision::Blocked(_)));
        gate.applied(r);
        assert_eq!(gate.decision(), Decision::Execute);
    }
    #[test]
    fn queued_keys_from_a_file_dialog_cannot_run_after_it_closes() {
        let mut gate = InputGate::default();
        let r = gate.request_apply();
        gate.connected = true;
        gate.applied(r);
        assert_eq!(
            gate.decision_for_age(std::time::Duration::from_secs(2)),
            Decision::Blocked("Appui ancien ignoré")
        );
        assert_eq!(
            gate.decision_for_age(std::time::Duration::from_millis(30)),
            Decision::Execute
        );
    }
}
