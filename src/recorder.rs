use eframe::egui::{Event, Key};
pub fn combination(events: &[Event]) -> Option<Result<String, String>> {
    for event in events {
        if let Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers,
            ..
        } = event
        {
            let key = match key {
                Key::ArrowDown => "DOWN",
                Key::ArrowUp => "UP",
                Key::ArrowLeft => "LEFT",
                Key::ArrowRight => "RIGHT",
                Key::Escape => "ESC",
                Key::PageUp => "PAGEUP",
                Key::PageDown => "PAGEDOWN",
                Key::Plus => "PLUS",
                Key::Minus => "MINUS",
                Key::Comma => "COMMA",
                Key::Period => "PERIOD",
                Key::Slash => "SLASH",
                Key::Backslash => "BACKSLASH",
                Key::Semicolon => "SEMICOLON",
                Key::Quote => "QUOTE",
                Key::OpenBracket => "LBRACKET",
                Key::CloseBracket => "RBRACKET",
                Key::Backtick => "BACKTICK",
                other => other.name(),
            };
            let mut parts = vec![];
            if modifiers.ctrl {
                parts.push("CTRL");
            }
            if modifiers.alt {
                parts.push("ALT");
            }
            if modifiers.shift {
                parts.push("SHIFT");
            }
            if modifiers.mac_cmd {
                parts.push("WIN");
            }
            parts.push(key);
            let text = parts.join("+");
            return Some(
                crate::model::parse_hotkey(&text)
                    .map(|_| text)
                    .map_err(|e| e.to_string()),
            );
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Modifiers;
    #[test]
    fn records_combination_and_ignores_key_repeats_and_release() {
        let event = Event::Key {
            key: Key::S,
            physical_key: Some(Key::S),
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
        };
        assert_eq!(combination(&[event]).unwrap().unwrap(), "CTRL+SHIFT+S");
        let repeated = Event::Key {
            key: Key::S,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        };
        assert!(combination(&[repeated]).is_none());
        let release = Event::Key {
            key: Key::S,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert!(combination(&[release]).is_none());
    }
}
