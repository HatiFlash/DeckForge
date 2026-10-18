use std::time::Duration;

#[derive(Default)]
pub struct Journal {
    pub open: bool,
    pub confirm_real: bool,
    pub mode: Option<bool>, // true: simulation; false: real execution
    pub finished: bool,
    pub elapsed: Duration,
    pub lines: Vec<Line>,
    pub error: Option<String>,
}

pub struct Line {
    pub path: Option<Vec<usize>>,
    pub message: String,
    pub elapsed: Option<Duration>,
}

impl Journal {
    pub fn begin(&mut self, dry: bool) {
        *self = Self {
            mode: Some(dry),
            ..Self::default()
        };
    }
    pub fn step(&mut self, path: Vec<usize>, name: String, elapsed: Option<Duration>) {
        if elapsed.is_none() && self.lines.len() < 500 {
            self.lines.push(Line {
                path: Some(path),
                message: name,
                elapsed: None,
            });
        } else if let Some(duration) = elapsed {
            if let Some(line) = self
                .lines
                .iter_mut()
                .rev()
                .find(|line| line.path.as_ref() == Some(&path) && line.elapsed.is_none())
            {
                line.elapsed = Some(duration);
            }
        }
        self.open = true;
    }
    pub fn log(&mut self, message: &str) {
        if self.mode.is_some() && !self.finished && self.lines.len() < 500 {
            self.lines.push(Line {
                path: None,
                message: message.into(),
                elapsed: None,
            });
        }
    }
    pub fn finish(&mut self, ok: bool, elapsed: Duration, message: String) {
        self.finished = true;
        self.elapsed = elapsed;
        if !ok {
            self.error = Some(message.clone());
        }
        if self.lines.len() < 500 {
            self.lines.push(Line {
                path: None,
                message,
                elapsed: None,
            });
        }
        self.open = true;
    }
    pub fn summary(&self) -> String {
        let prefix = if self.mode == Some(false) {
            "EXÉCUTION RÉELLE"
        } else {
            "Simulation"
        };
        if self.mode.is_none() {
            return "Simulation · Aucun test effectué".into();
        }
        if !self.finished {
            return format!("{prefix} · En cours");
        }
        if self.error.is_some() {
            let last = self.lines.iter().rev().find_map(|line| line.path.as_ref());
            return match last {
                Some(path) => format!(
                    "{prefix} · Échec à l'étape {}",
                    path.iter()
                        .step_by(2)
                        .map(|i| (i + 1).to_string())
                        .collect::<Vec<_>>()
                        .join(".")
                ),
                None => format!("{prefix} · Échec"),
            };
        }
        let count = self.lines.iter().filter(|line| line.path.is_some()).count();
        format!(
            "{prefix} · {count} étapes · OK · {:.1} s",
            self.elapsed.as_secs_f64()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn events_aggregate_success_failure_and_empty_state() {
        let mut j = Journal::default();
        assert_eq!(j.summary(), "Simulation · Aucun test effectué");
        assert!(!j.open);
        j.begin(true);
        j.step(vec![0], "Attendre".into(), None);
        j.step(vec![0], "Attendre".into(), Some(Duration::from_millis(12)));
        j.step(vec![1], "Texte".into(), None);
        j.finish(true, Duration::from_millis(1200), "Macro terminée".into());
        assert_eq!(j.summary(), "Simulation · 2 étapes · OK · 1.2 s");
        assert!(j.open);
        j.begin(false);
        j.step(vec![2], "Erreur".into(), None);
        j.finish(
            false,
            Duration::from_millis(2),
            "Macro arrêtée : erreur".into(),
        );
        assert_eq!(j.summary(), "EXÉCUTION RÉELLE · Échec à l'étape 3");
    }
}
