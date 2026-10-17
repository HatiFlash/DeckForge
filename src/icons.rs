use crate::model::{Key, Step};
pub fn kind(key: &Key) -> usize {
    if let Some(kind) = key.display_kind {
        return kind;
    }
    if key.steps.len() > 1 {
        25
    } else {
        key.steps.first().map(step_kind).unwrap_or(8)
    }
}
pub fn step_kind(step: &Step) -> usize {
    match step.active() {
        Step::Disabled { .. } => unreachable!(),
        Step::MouseClick { .. } | Step::MouseMove { .. } | Step::MouseScroll { .. } => 1,
        Step::PlaySound { .. } | Step::StopSounds => 6,
        Step::Toggle { .. } => 30,
        Step::UdpSend { .. } | Step::HttpRequest { .. } => 4,
        Step::D6Brightness { .. } | Step::D6Sleep => 9,
        Step::CloseWindow => 5,
        Step::Delay { .. } => 0,
        Step::Hotkey { .. } => 1,
        Step::Text { .. } => 2,
        Step::Launch { .. } => 3,
        Step::Url { .. } => 4,
        Step::WaitWindow { .. } | Step::WaitUntil { .. } => 5,
        Step::Notify { .. } => 6,
        Step::Profile { .. } => 7,
        Step::Media { command } => match command.as_str() {
            "volume_up" => 9,
            "volume_down" => 10,
            "mute" => 11,
            "next" => 12,
            "previous" => 13,
            "stop" => 14,
            _ => 6,
        },
        Step::AppVolume { .. } | Step::DeviceVolume { .. } => 9,
        Step::AudioSwitch { input, .. } => {
            if *input {
                28
            } else {
                26
            }
        }
        Step::IfWindow { .. } | Step::IfVariable { .. } | Step::IfCondition { .. } => 29,
        Step::Repeat { .. } => 30,
        Step::SetVariable { .. } | Step::Math { .. } => 2,
        Step::AutoHotkey { .. } => 1,
        Step::ObsScene { .. } | Step::ObsRecord { .. } | Step::ObsStream { .. } | Step::ObsVirtualCam { .. } | Step::ObsReplay { .. } | Step::ObsMute { .. } | Step::ObsSourceVisibility { .. } => 6,
        Step::Folder { .. } => 15,
        Step::Back => 16,
        Step::Page { .. } => 17,
        Step::PageNext => 18,
        Step::PagePrevious => 19,
    }
}
/// Small white line icons rendered directly into the LCD bitmap.
pub fn draw(image: &mut image::RgbImage, kind: usize) {
    let size = image.width() as f32;
    let center = (size * 0.5, size * 0.29);
    let scale = size * 0.15;
    let mut line = |a: (f32, f32), b: (f32, f32)| {
        let ax = center.0 + a.0 * scale;
        let ay = center.1 + a.1 * scale;
        let bx = center.0 + b.0 * scale;
        let by = center.1 + b.1 * scale;
        let dx = bx - ax;
        let dy = by - ay;
        let length = dx * dx + dy * dy;
        for y in ((ay.min(by) - 2.0).max(0.0) as u32)..=((ay.max(by) + 2.0).min(size - 1.0) as u32)
        {
            for x in
                ((ax.min(bx) - 2.0).max(0.0) as u32)..=((ax.max(bx) + 2.0).min(size - 1.0) as u32)
            {
                let t = if length == 0.0 {
                    0.0
                } else {
                    (((x as f32 - ax) * dx + (y as f32 - ay) * dy) / length).clamp(0.0, 1.0)
                };
                let distance =
                    ((x as f32 - ax - t * dx).powi(2) + (y as f32 - ay - t * dy).powi(2)).sqrt();
                let alpha = (2.1 - distance).clamp(0.0, 1.0);
                if alpha > 0.0 {
                    for ch in &mut image.get_pixel_mut(x, y).0 {
                        *ch = (*ch as f32 * (1.0 - alpha) + 255.0 * alpha) as u8;
                    }
                }
            }
        }
    };
    let mut segments = Vec::<((f32, f32), (f32, f32))>::new();
    let mut rect = |x: f32, y: f32, w: f32, h: f32| {
        segments.extend([
            ((x, y), (x + w, y)),
            ((x + w, y), (x + w, y + h)),
            ((x + w, y + h), (x, y + h)),
            ((x, y + h), (x, y)),
        ]);
    };
    match kind {
        26 => {
            rect(-1.0, -0.1, 0.4, 1.0);
            rect(0.6, -0.1, 0.4, 1.0);
            for i in 0..16 {
                let a = std::f32::consts::PI + i as f32 * std::f32::consts::PI / 16.0;
                let b = a + std::f32::consts::PI / 16.0;
                segments.push(((a.cos(), a.sin()), (b.cos(), b.sin())));
            }
        }
        27 => {
            rect(-0.65, -1.0, 1.3, 2.0);
            rect(-0.25, -0.6, 0.5, 0.3);
            rect(-0.35, 0.15, 0.7, 0.7);
        }
        28 => {
            rect(-0.35, -1.0, 0.7, 1.2);
            segments.extend([
                ((-0.7, -0.1), (-0.7, 0.5)),
                ((-0.7, 0.5), (0.7, 0.5)),
                ((0.7, 0.5), (0.7, -0.1)),
                ((0.0, 0.5), (0.0, 1.0)),
                ((-0.5, 1.0), (0.5, 1.0)),
            ]);
        }
        29 => {
            segments.extend([
                ((0.0, -1.0), (1.0, 0.0)),
                ((1.0, 0.0), (0.0, 1.0)),
                ((0.0, 1.0), (-1.0, 0.0)),
                ((-1.0, 0.0), (0.0, -1.0)),
            ]);
        }
        30 => {
            segments.extend([
                ((-1.0, 0.0), (-1.0, -0.7)),
                ((-1.0, -0.7), (1.0, -0.7)),
                ((1.0, -0.7), (1.0, 0.7)),
                ((1.0, 0.7), (-1.0, 0.7)),
            ]);
        }
        0 | 4 => {
            for i in 0..32 {
                let a = i as f32 * std::f32::consts::TAU / 32.0;
                let b = (i + 1) as f32 * std::f32::consts::TAU / 32.0;
                segments.push(((a.cos(), a.sin()), (b.cos(), b.sin())));
            }
            if kind == 0 {
                segments.extend([((0.0, -0.65), (0.0, 0.0)), ((0.0, 0.0), (0.5, 0.3))]);
            } else {
                segments.extend([((-1.0, 0.0), (1.0, 0.0)), ((0.0, -1.0), (0.0, 1.0))]);
            }
        }
        1 => {
            rect(-1.0, -0.7, 2.0, 1.4);
            for y in [-0.3, 0.2] {
                for x in [-0.6, 0.0, 0.6] {
                    segments.push(((x, y), (x + 0.1, y)));
                }
            }
        }
        2 => {
            for y in [-0.65, 0.0, 0.65] {
                segments.push(((-0.9, y), (0.9, y)));
            }
        }
        3 | 5 | 17 => {
            rect(-0.85, -0.9, 1.7, 1.8);
            segments.push(((-0.85, -0.4), (0.85, -0.4)));
        }
        6 | 12 | 13 => {
            let sign = if kind == 13 { -1.0 } else { 1.0 };
            segments.extend([
                ((-sign * 0.6, -0.9), (sign * 0.8, 0.0)),
                ((sign * 0.8, 0.0), (-sign * 0.6, 0.9)),
                ((-sign * 0.6, 0.9), (-sign * 0.6, -0.9)),
            ]);
            if kind != 6 {
                segments.push(((sign, -0.9), (sign, 0.9)));
            }
        }
        7 | 25 => {
            rect(-1.0, -1.0, 1.5, 1.5);
            rect(-0.5, -0.5, 1.5, 1.5);
        }
        9..=11 => {
            segments.extend([
                ((-1.0, -0.35), (-0.5, -0.35)),
                ((-0.5, -0.35), (0.1, -0.8)),
                ((0.1, -0.8), (0.1, 0.8)),
                ((0.1, 0.8), (-0.5, 0.35)),
                ((-0.5, 0.35), (-1.0, 0.35)),
                ((-1.0, 0.35), (-1.0, -0.35)),
            ]);
            if kind == 11 {
                segments.extend([((0.4, -0.4), (1.0, 0.4)), ((0.4, 0.4), (1.0, -0.4))]);
            } else {
                segments.push(((0.4, 0.0), (1.0, 0.0)));
                if kind == 9 {
                    segments.push(((0.7, -0.3), (0.7, 0.3)));
                }
            }
        }
        14 => rect(-0.7, -0.7, 1.4, 1.4),
        15 => {
            rect(-1.0, -0.45, 2.0, 1.3);
            segments.extend([
                ((-1.0, -0.45), (-1.0, -0.85)),
                ((-1.0, -0.85), (-0.2, -0.85)),
                ((-0.2, -0.85), (0.0, -0.45)),
            ]);
        }
        16 => segments.extend([
            ((0.9, 0.0), (-0.8, 0.0)),
            ((-0.8, 0.0), (-0.1, -0.7)),
            ((-0.8, 0.0), (-0.1, 0.7)),
        ]),
        18 | 19 => {
            let sign = if kind == 18 { 1.0 } else { -1.0 };
            segments.extend([
                ((-sign * 0.3, -0.8), (sign * 0.5, 0.0)),
                ((sign * 0.5, 0.0), (-sign * 0.3, 0.8)),
            ]);
        }
        _ => segments.extend([((-0.8, 0.0), (0.8, 0.0)), ((0.0, -0.8), (0.0, 0.8))]),
    }
    for (a, b) in segments {
        line(a, b);
    }
}
