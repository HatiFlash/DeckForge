use eframe::egui::{Event, Key, PointerButton, Pos2, Rect};

/// Keep the press origin, even when several mouse events arrive in one frame.
pub struct Drag<T> {
    pub active: Option<(T, Pos2)>,
    pub position: Option<Pos2>,
}
impl<T> Default for Drag<T> {
    fn default() -> Self {
        Self {
            active: None,
            position: None,
        }
    }
}
impl<T: Clone> Drag<T> {
    pub fn cancel(&mut self) {
        self.active = None;
    }
    pub fn moving(&self) -> bool {
        self.active
            .as_ref()
            .zip(self.position)
            .is_some_and(|((_, start), pos)| start.distance(pos) > 6.0)
    }
    pub fn update(
        &mut self,
        events: &[Event],
        sources: &[(Rect, T)],
        targets: &[(Rect, usize)],
    ) -> Option<(T, usize)> {
        let mut dropped = None;
        for event in events {
            match event {
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    ..
                } => {
                    self.position = Some(*pos);
                    self.active = sources
                        .iter()
                        .rev()
                        .find(|(rect, _)| rect.contains(*pos))
                        .map(|(_, value)| (value.clone(), *pos));
                }
                Event::PointerMoved(pos) => self.position = Some(*pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    ..
                } => {
                    self.position = Some(*pos);
                    if self.moving() {
                        if let Some((_, target)) =
                            targets.iter().find(|(rect, _)| rect.contains(*pos))
                        {
                            if let Some((item, _)) = self.active.take() {
                                dropped = Some((item, *target));
                            }
                        }
                    }
                    self.cancel();
                }
                Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                }
                | Event::PointerGone => self.cancel(),
                _ => {}
            }
        }
        dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{pos2, vec2, Modifiers};
    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }
    #[test]
    fn fast_drag_in_one_frame_keeps_catalog_origin() {
        let mut drag = Drag::default();
        let sources = [
            (
                Rect::from_min_size(pos2(200.0, 0.0), vec2(100.0, 50.0)),
                "catalog",
            ),
            (
                Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)),
                "key",
            ),
        ];
        let targets = [(sources[1].0, 7)];
        let result = drag.update(
            &[
                button(pos2(220.0, 20.0), true),
                Event::PointerMoved(pos2(40.0, 40.0)),
                button(pos2(40.0, 40.0), false),
            ],
            &sources,
            &targets,
        );
        assert_eq!(result, Some(("catalog", 7)));
        assert!(drag.active.is_none());
    }
    #[test]
    fn click_escape_and_drop_outside_do_not_assign_a_key() {
        let mut drag = Drag::default();
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0));
        let sources = [(rect, 42)];
        let targets = [(rect, 0)];
        assert!(drag
            .update(
                &[
                    button(pos2(20.0, 20.0), true),
                    button(pos2(21.0, 21.0), false)
                ],
                &sources,
                &targets
            )
            .is_none());
        drag.update(
            &[
                button(pos2(20.0, 20.0), true),
                Event::PointerMoved(pos2(50.0, 50.0)),
            ],
            &sources,
            &targets,
        );
        assert!(drag.moving());
        drag.cancel();
        assert!(drag
            .update(&[button(pos2(50.0, 50.0), false)], &sources, &targets)
            .is_none());
        assert!(drag
            .update(
                &[
                    button(pos2(20.0, 20.0), true),
                    button(pos2(200.0, 200.0), false)
                ],
                &sources,
                &targets
            )
            .is_none());
    }
}
