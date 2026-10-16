use eframe::egui::{self, Color32, Stroke, Vec2};
pub const BLUE: Color32 = Color32::from_rgb(69, 143, 255);
pub const GREEN: Color32 = Color32::from_rgb(55, 181, 135);
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
pub fn icon_color(color: Color32, dark: bool) -> Color32 {
    let luminance =
        0.2126 * color.r() as f32 + 0.7152 * color.g() as f32 + 0.0722 * color.b() as f32;
    if dark && luminance < 130.0 {
        let lift = (150.0 - luminance) as u8;
        Color32::from_rgb(
            color.r().saturating_add(lift),
            color.g().saturating_add(lift),
            color.b().saturating_add(lift),
        )
    } else {
        color
    }
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
    style.visuals.window_corner_radius = 10.into();
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = 6.into();
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
