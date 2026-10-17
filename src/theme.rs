use eframe::egui::{self, Color32, Stroke, Vec2};
pub const BLUE: Color32 = Color32::from_rgb(64, 145, 255);
pub const GREEN: Color32 = Color32::from_rgb(55, 181, 135);
pub fn app_icon() -> egui::IconData {
    let mut rgba = vec![0; 32 * 32 * 4];
    for y in 0..32usize {
        for x in 0..32usize {
            let dx = if x < 7 { 7 - x } else { x.saturating_sub(24) };
            let dy = if y < 7 { 7 - y } else { y.saturating_sub(24) };
            if dx * dx + dy * dy <= 49 {
                let p = (y * 32 + x) * 4;
                rgba[p..p + 4].copy_from_slice(&[54, 139, 250, 255]);
            }
        }
    }
    for row in 0..3 {
        for col in 0..5 {
            for y in 0..3 {
                for x in 0..3 {
                    let p = ((9 + row * 6 + y) * 32 + 3 + col * 5 + x) * 4;
                    rgba[p..p + 4].copy_from_slice(&[244, 249, 255, 255]);
                }
            }
        }
    }
    egui::IconData {
        rgba,
        width: 32,
        height: 32,
    }
}
pub fn rounded_gradient(
    p: &egui::Painter,
    r: egui::Rect,
    radius: f32,
    top: Color32,
    bottom: Color32,
) {
    let mut mesh = egui::Mesh::default();
    let color = |y: f32| {
        let t = ((y - r.top()) / r.height()).clamp(0.0, 1.0);
        Color32::from_rgb(
            (top.r() as f32 * (1.0 - t) + bottom.r() as f32 * t) as u8,
            (top.g() as f32 * (1.0 - t) + bottom.g() as f32 * t) as u8,
            (top.b() as f32 * (1.0 - t) + bottom.b() as f32 * t) as u8,
        )
    };
    mesh.colored_vertex(r.center(), color(r.center().y));
    for (center, start) in [
        (r.left_top() + Vec2::splat(radius), std::f32::consts::PI),
        (
            r.right_top() + Vec2::new(-radius, radius),
            std::f32::consts::PI * 1.5,
        ),
        (r.right_bottom() - Vec2::splat(radius), 0.0),
        (
            r.left_bottom() + Vec2::new(radius, -radius),
            std::f32::consts::FRAC_PI_2,
        ),
    ] {
        for i in 0..=8 {
            let angle = start + i as f32 * std::f32::consts::FRAC_PI_2 / 8.0;
            let pos = center + Vec2::angled(angle) * radius;
            mesh.colored_vertex(pos, color(pos.y));
        }
    }
    let n = mesh.vertices.len() as u32 - 1;
    for i in 1..=n {
        mesh.add_triangle(0, i, if i == n { 1 } else { i + 1 });
    }
    p.add(egui::Shape::mesh(mesh));
}
pub fn device_shell(p: &egui::Painter, r: egui::Rect) {
    for offset in (1..=12).rev() {
        p.rect_filled(
            r.translate(Vec2::new(0.0, offset as f32)),
            24.0,
            Color32::from_rgb(9 + offset, 10 + offset, 12 + offset),
        );
    }
    rounded_gradient(
        p,
        r,
        24.0,
        Color32::from_rgb(102, 106, 113),
        Color32::from_rgb(38, 40, 46),
    );
    rounded_gradient(
        p,
        r.shrink(2.0),
        23.0,
        Color32::from_rgb(50, 53, 59),
        Color32::from_rgb(20, 22, 27),
    );
    rounded_gradient(
        p,
        r.shrink(7.0),
        19.0,
        Color32::from_rgb(30, 32, 38),
        Color32::from_rgb(24, 26, 31),
    );
    p.rect_stroke(
        r.shrink(8.0),
        19.0,
        Stroke::new(1.0_f32, Color32::from_black_alpha(150)),
        egui::StrokeKind::Inside,
    );
    p.line_segment(
        [
            r.left_top() + Vec2::new(27.0, 1.0),
            r.right_top() + Vec2::new(-27.0, 1.0),
        ],
        Stroke::new(1.0_f32, Color32::from_white_alpha(110)),
    );
    for x in [r.left() + 13.0, r.right() - 13.0] {
        for y in [r.top() + 14.0, r.bottom() - 14.0] {
            p.circle_filled(egui::pos2(x, y), 1.5, Color32::from_rgb(15, 16, 19));
        }
    }
}
pub fn key_face(p: &egui::Painter, r: egui::Rect, selected: bool, flashing: bool) {
    p.rect_filled(r.expand(3.0), 12.0, Color32::from_rgb(8, 9, 12));
    p.rect_filled(
        r.translate(Vec2::new(0.0, 4.0)),
        10.0,
        Color32::from_rgb(7, 8, 10),
    );
    rounded_gradient(
        p,
        r,
        10.0,
        Color32::from_rgb(74, 77, 85),
        Color32::from_rgb(34, 36, 42),
    );
    rounded_gradient(
        p,
        r.shrink(2.0),
        8.0,
        if selected {
            Color32::from_rgb(37, 64, 94)
        } else {
            Color32::from_rgb(40, 43, 51)
        },
        if selected {
            Color32::from_rgb(25, 45, 67)
        } else {
            Color32::from_rgb(28, 31, 37)
        },
    );
    p.line_segment(
        [
            r.left_top() + Vec2::new(10.0, 1.0),
            r.right_top() + Vec2::new(-10.0, 1.0),
        ],
        Stroke::new(1.0_f32, Color32::from_white_alpha(65)),
    );
    if selected || flashing {
        p.rect_stroke(
            r,
            10.0,
            Stroke::new(2.0_f32, if flashing { GREEN } else { BLUE }),
            egui::StrokeKind::Inside,
        );
    }
}
pub fn gradient(p: &egui::Painter, r: egui::Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), top);
    mesh.colored_vertex(r.right_top(), top);
    mesh.colored_vertex(r.right_bottom(), bottom);
    mesh.colored_vertex(r.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
}
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub wash: Color32,
    pub tile: Color32,
}
pub fn palette(ctx: &egui::Context) -> Palette {
    if ctx.style().visuals.dark_mode {
        Palette {
            bg: Color32::from_rgb(24, 26, 31),
            surface: Color32::from_rgb(32, 35, 42),
            ink: Color32::from_rgb(234, 236, 242),
            muted: Color32::from_rgb(158, 164, 178),
            line: Color32::from_rgb(54, 59, 70),
            wash: Color32::from_rgb(38, 63, 96),
            tile: Color32::from_rgb(43, 47, 57),
        }
    } else {
        Palette {
            bg: Color32::from_rgb(237, 240, 245),
            surface: Color32::from_rgb(250, 251, 253),
            ink: Color32::from_rgb(28, 32, 41),
            muted: Color32::from_rgb(99, 108, 124),
            line: Color32::from_rgb(209, 215, 225),
            wash: Color32::from_rgb(218, 233, 255),
            tile: Color32::WHITE,
        }
    }
}
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let windir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
    if let Ok(bytes) = std::fs::read(std::path::PathBuf::from(windir).join("Fonts/segoeui.ttf")) {
        fonts
            .font_data
            .insert("system-ui".into(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "system-ui".into());
    }
    ctx.set_fonts(fonts);
    apply(ctx, true);
}
pub fn apply(ctx: &egui::Context, dark: bool) {
    ctx.set_visuals(if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    });
    let p = palette(ctx);
    let mut style = (*ctx.style()).clone();
    style.visuals.panel_fill = p.bg;
    style.visuals.window_fill = p.surface;
    style.visuals.extreme_bg_color = p.bg;
    style.visuals.override_text_color = Some(p.ink);
    style.visuals.faint_bg_color = p.bg;
    style.visuals.selection.bg_fill = p.wash;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, BLUE);
    style.visuals.window_corner_radius = 16.into();
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = 9.into();
        widget.bg_stroke = Stroke::new(1.0_f32, p.line);
        widget.fg_stroke = Stroke::new(1.0_f32, p.ink);
    }
    style.visuals.widgets.inactive.weak_bg_fill = p.tile;
    style.visuals.widgets.hovered.weak_bg_fill = p.wash;
    style.visuals.widgets.active.weak_bg_fill = p.wash;
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.interact_size.y = 32.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(26.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
    ctx.set_style(style);
}
pub fn primary(text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    egui::Button::new(text)
        .fill(BLUE)
        .stroke(Stroke::NONE)
        .corner_radius(6)
}
pub fn pill(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.09))
        .corner_radius(20)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).color(color).size(11.0).strong());
        });
}
pub fn toggle(ui: &mut egui::Ui, value: &mut bool) -> egui::Response {
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(46.0, 26.0), egui::Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool(response.id, *value);
    ui.painter().rect_filled(
        rect,
        13.0,
        if *value {
            BLUE
        } else {
            Color32::from_rgb(201, 205, 214)
        },
    );
    let center = egui::pos2(
        egui::lerp((rect.left() + 13.0)..=(rect.right() - 13.0), t),
        rect.center().y,
    );
    ui.painter().circle_filled(center, 10.0, Color32::WHITE);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            ui.is_enabled(),
            *value,
            "Touches actives",
        )
    });
    response
}
pub fn symbol(p: &egui::Painter, c: egui::Pos2, size: f32, kind: usize, color: Color32) {
    let s = size * 0.5;
    let stroke = Stroke::new(1.8_f32, color);
    if kind >= 26 {
        let mut bitmap = image::RgbImage::new(80, 80);
        crate::icons::draw(&mut bitmap, kind);
        // The LCD icon renderer is shared with the desktop preview.
        let mut mesh = egui::Mesh::default();
        for y in 4..42 {
            for x in 20..61 {
                let alpha = bitmap.get_pixel(x, y).0[0];
                if alpha > 0 {
                    let pos = c + Vec2::new(
                        (x as f32 - 40.0) * size / 25.0,
                        (y as f32 - 23.2) * size / 25.0,
                    );
                    mesh.add_colored_rect(
                        egui::Rect::from_min_size(pos, Vec2::splat(size / 25.0 + 0.2)),
                        color.gamma_multiply(alpha as f32 / 255.0),
                    );
                }
            }
        }
        p.add(egui::Shape::mesh(mesh));
        return;
    }
    match kind {
        25 => {
            p.rect_stroke(
                egui::Rect::from_center_size(
                    c + Vec2::new(-s * 0.2, -s * 0.2),
                    Vec2::splat(size * 0.7),
                ),
                3.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                egui::Rect::from_center_size(
                    c + Vec2::new(s * 0.2, s * 0.2),
                    Vec2::splat(size * 0.7),
                ),
                3.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        15 => {
            p.rect_stroke(
                egui::Rect::from_center_size(
                    c + Vec2::new(0.0, s * 0.13),
                    Vec2::new(size, size * 0.66),
                ),
                3.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.line_segment(
                [
                    c + Vec2::new(-s, -s * 0.45),
                    c + Vec2::new(-s * 0.2, -s * 0.45),
                ],
                stroke,
            );
        }
        16 => {
            p.line_segment(
                [c + Vec2::new(s * 0.8, 0.0), c + Vec2::new(-s * 0.7, 0.0)],
                stroke,
            );
            p.line_segment(
                [
                    c + Vec2::new(-s * 0.7, 0.0),
                    c + Vec2::new(-s * 0.1, -s * 0.6),
                ],
                stroke,
            );
            p.line_segment(
                [
                    c + Vec2::new(-s * 0.7, 0.0),
                    c + Vec2::new(-s * 0.1, s * 0.6),
                ],
                stroke,
            );
        }
        17 => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, Vec2::new(size * 0.75, size)),
                3.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            for y in [-0.35, 0.0, 0.35] {
                p.line_segment(
                    [
                        c + Vec2::new(-s * 0.4, s * y),
                        c + Vec2::new(s * 0.4, s * y),
                    ],
                    stroke,
                );
            }
        }
        18 | 19 => {
            let sign = if kind == 18 { 1.0 } else { -1.0 };
            p.line_segment(
                [
                    c + Vec2::new(-sign * s * 0.25, -s * 0.65),
                    c + Vec2::new(sign * s * 0.45, 0.0),
                ],
                stroke,
            );
            p.line_segment(
                [
                    c + Vec2::new(sign * s * 0.45, 0.0),
                    c + Vec2::new(-sign * s * 0.25, s * 0.65),
                ],
                stroke,
            );
        }
        20 => {
            p.rect_stroke(
                egui::Rect::from_center_size(
                    c + Vec2::new(0.0, s * 0.2),
                    Vec2::new(size * 0.8, size * 0.7),
                ),
                3.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.circle_stroke(c + Vec2::new(0.0, -s * 0.4), s * 0.35, stroke);
        }
        21 => {
            for y in [-0.3, 0.0, 0.3] {
                p.line_segment(
                    [
                        c + Vec2::new(-s * 0.8, y * s),
                        c + Vec2::new(s * 0.8, y * s),
                    ],
                    stroke,
                );
            }
        }
        0 => {
            p.circle_stroke(c, s * 0.8, stroke);
            p.line_segment([c, c + Vec2::new(0.0, -s * 0.45)], stroke);
            p.line_segment([c, c + Vec2::new(s * 0.38, s * 0.18)], stroke);
        }
        1 => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, Vec2::new(size, size * 0.66)),
                3.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            for y in [-0.2, 0.2] {
                for x in [-0.45, 0.0, 0.45] {
                    p.circle_filled(c + Vec2::new(x * s, y * s), 1.0, color);
                }
            }
        }
        2 => {
            for y in [-0.5, 0.0, 0.5] {
                p.line_segment(
                    [
                        c + Vec2::new(-s * 0.7, y * s),
                        c + Vec2::new(s * 0.7, y * s),
                    ],
                    stroke,
                );
            }
        }
        3 => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, Vec2::splat(size * 0.8)),
                4.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.line_segment(
                [
                    c + Vec2::new(-s * 0.7, -s * 0.25),
                    c + Vec2::new(s * 0.7, -s * 0.25),
                ],
                stroke,
            );
        }
        4 => {
            p.circle_stroke(c, s * 0.85, stroke);
            p.line_segment(
                [c + Vec2::new(-s * 0.85, 0.0), c + Vec2::new(s * 0.85, 0.0)],
                stroke,
            );
            p.add(egui::epaint::EllipseShape::stroke(
                c,
                Vec2::new(size * 0.2, size * 0.425),
                stroke,
            ));
        }
        5 => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, Vec2::new(size, size * 0.7)),
                3.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.line_segment(
                [
                    c + Vec2::new(-s * 0.35, s * 0.95),
                    c + Vec2::new(s * 0.35, s * 0.95),
                ],
                stroke,
            );
        }
        6 => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + Vec2::new(-s * 0.45, -s * 0.75),
                    c + Vec2::new(s * 0.8, 0.0),
                    c + Vec2::new(-s * 0.45, s * 0.75),
                ],
                color,
                Stroke::NONE,
            ));
        }
        7 => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, Vec2::new(size, size * 0.7)),
                3.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.line_segment(
                [
                    c + Vec2::new(-s * 0.8, -s * 0.7),
                    c + Vec2::new(-s * 0.2, -s * 0.7),
                ],
                stroke,
            );
        }
        9 | 10 | 11 => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + Vec2::new(-s * 0.85, -s * 0.3),
                    c + Vec2::new(-s * 0.45, -s * 0.3),
                    c + Vec2::new(s * 0.1, -s * 0.75),
                    c + Vec2::new(s * 0.1, s * 0.75),
                    c + Vec2::new(-s * 0.45, s * 0.3),
                    c + Vec2::new(-s * 0.85, s * 0.3),
                ],
                color,
                Stroke::NONE,
            ));
            if kind == 11 {
                p.line_segment(
                    [
                        c + Vec2::new(s * 0.35, -s * 0.4),
                        c + Vec2::new(s * 0.85, s * 0.4),
                    ],
                    stroke,
                );
                p.line_segment(
                    [
                        c + Vec2::new(s * 0.35, s * 0.4),
                        c + Vec2::new(s * 0.85, -s * 0.4),
                    ],
                    stroke,
                );
            } else {
                p.line_segment(
                    [c + Vec2::new(s * 0.35, 0.0), c + Vec2::new(s * 0.9, 0.0)],
                    stroke,
                );
                if kind == 9 {
                    p.line_segment(
                        [
                            c + Vec2::new(s * 0.625, -s * 0.3),
                            c + Vec2::new(s * 0.625, s * 0.3),
                        ],
                        stroke,
                    );
                }
            }
        }
        12 | 13 => {
            let sign = if kind == 12 { 1.0 } else { -1.0 };
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + Vec2::new(-sign * s * 0.6, -s * 0.7),
                    c + Vec2::new(sign * s * 0.5, 0.0),
                    c + Vec2::new(-sign * s * 0.6, s * 0.7),
                ],
                color,
                Stroke::NONE,
            ));
            p.line_segment(
                [
                    c + Vec2::new(sign * s * 0.8, -s * 0.7),
                    c + Vec2::new(sign * s * 0.8, s * 0.7),
                ],
                stroke,
            );
        }
        14 => {
            p.rect_filled(
                egui::Rect::from_center_size(c, Vec2::splat(size * 0.65)),
                2.0,
                color,
            );
        }
        _ => {
            p.line_segment(
                [c + Vec2::new(-s * 0.55, 0.0), c + Vec2::new(s * 0.55, 0.0)],
                stroke,
            );
            p.line_segment(
                [c + Vec2::new(0.0, -s * 0.55), c + Vec2::new(0.0, s * 0.55)],
                stroke,
            );
        }
    }
}
