//! One read-only worker monitors all keys of the visible page, outside App's mutex.
use crate::{conditions::Condition, model::Key, Event};
use std::{collections::BTreeMap, sync::mpsc::{self, Sender, Receiver}, time::{Duration, Instant}};

pub enum Command { Page(u64, Vec<Key>) }

/// Pure change detector: duplicate observations never generate uploads.
pub fn changed(previous: &mut Option<bool>, observed: bool) -> bool {
    if *previous == Some(observed) { false } else { *previous = Some(observed); true }
}

pub fn spawn(events: Sender<Event>) -> Sender<Command> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || monitor(rx, events));
    tx
}
fn monitor(rx: Receiver<Command>, events: Sender<Event>) {
    let mut generation = 0;
    let mut keys: Vec<(usize, Condition, Duration, Instant, Option<bool>)> = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Command::Page(id, page)) => {
                generation = id;
                keys = page.into_iter().enumerate().filter_map(|(index,key)| {
                    key.state_display.map(|display| (index, display.condition,
                        Duration::from_millis(display.poll_ms.max(250)), Instant::now(), None))
                }).collect();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let now = Instant::now();
        for (index, condition, interval, due, previous) in &mut keys {
            if now < *due { continue; }
            *due = now + *interval;
            match condition.evaluate(&BTreeMap::new()) {
                Ok(value) if changed(previous, value) => { if events.send(Event::StateDisplay(generation, *index, value)).is_err() { return; } }
                Ok(_) => {}
                Err(err) => { if previous.take().is_some() {
                    if events.send(Event::Log(format!("Affichage selon l'état, touche {} : {err}", *index + 1))).is_err() { return; }
                } }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn unchanged_never_triggers_upload() {
        let mut state = None;
        assert!(changed(&mut state, false));
        assert!(!changed(&mut state, false));
        assert!(changed(&mut state, true));
        assert!(!changed(&mut state, true));
    }
}
