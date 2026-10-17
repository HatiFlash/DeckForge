use eframe::egui::{self, Color32, Stroke, Vec2};

pub const BLUE: Color32 = Color32::from_rgb(52, 120, 246);
pub const GREEN: Color32 = Color32::from_rgb(34, 197, 139);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(78, 141, 255);
pub const PURPLE: Color32 = Color32::from_rgb(139, 92, 246);
pub const CYAN: Color32 = Color32::from_rgb(34, 195, 238);
pub const WARNING: Color32 = Color32::from_rgb(245, 173, 66);
pub const DANGER: Color32 = Color32::from_rgb(242, 95, 112);

pub fn app_icon() -> egui::IconData {
    let mut rgba = vec![0; 32 * 32 * 4];
    for y in 0..32usize {
        for x in 0..32usize {
            let dx = if x < 7 { 7 - x } else { x.saturating_sub(24) };
            let dy = if y < 7 { 7 - y } else { y.saturating_sub(24) };
            if dx * dx + dy * dy <= 49 {
                let p = (y * 32 + x) * 4;
                rgba[p..p + 4].copy_from_slice(&[52, 120, 246, 255]);
            }
        }
    }
    for row in 0..3 {
        for col in 0..5 {
            for y in 0..3 {
                for x in 0..3 {
                    let p = ((9 + row * 6 + y) * 32 + 3 + col * 5 + x) * 4;
                    rgba[p..p + 4].copy_from_slice(&[241, 245, 251, 255]);
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
    p.rect_filled(
        r.expand(6.0),
        28.0,
        Color32::from_rgba_unmultiplied(52, 120, 246, 16),
    );
    p.rect_filled(
        r.translate(Vec2::new(0.0, 6.0)).expand(2.0),
        26.0,
        Color32::from_black_alpha(110),
    );
    rounded_gradient(
        p,
        r,
        24.0,
        Color32::from_rgb(24, 37, 60),
        Color32::from_rgb(13, 21, 36),
    );
    rounded_gradient(
        p,
        r.shrink(6.0),
        19.0,
        Color32::from_rgb(15, 24, 40),
        Color32::from_rgb(10, 16, 28),
    );
    p.rect_stroke(
        r,
        24.0,
        Stroke::new(1.0_f32, Color32::from_rgb(49, 65, 90)),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        r.shrink(6.0),
        19.0,
        Stroke::new(1.0_f32, Color32::from_black_alpha(140)),
        egui::StrokeKind::Inside,
    );
    p.line_segment(
        [
            r.left_top() + Vec2::new(24.0, 1.0),
            r.right_top() + Vec2::new(-24.0, 1.0),
        ],
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(167, 181, 204, 45)),
    );
}

pub fn key_face(p: &egui::Painter, r: egui::Rect, selected: bool, flashing: bool) {
    let outer_fill = if flashing {
        GREEN.gamma_multiply(0.22)
    } else if selected {
        CYAN.gamma_multiply(0.18)
    } else {
        Color32::from_rgb(8, 13, 24)
    };
    p.rect_filled(
        r.expand(if selected || flashing { 3.0 } else { 2.0 }),
        12.0,
        outer_fill,
    );

    p.rect_filled(
        r,
        10.0,
        if selected {
            Color32::from_rgb(23, 59, 121)
        } else {
            Color32::from_rgb(18, 28, 46)
        },
    );

    let (top, bottom) = if flashing {
        (Color32::from_rgb(22, 68, 62), Color32::from_rgb(15, 46, 44))
    } else if selected {
        (Color32::from_rgb(28, 63, 122), Color32::from_rgb(19, 44, 88))
    } else {
        (Color32::from_rgb(31, 46, 70), Color32::from_rgb(21, 32, 51))
    };
    rounded_gradient(p, r.shrink(1.5), 8.5, top, bottom);

    p.line_segment(
        [
            r.left_top() + Vec2::new(10.0, 1.5),
            r.right_top() + Vec2::new(-10.0, 1.5),
        ],
        Stroke::new(1.0_f32, Color32::from_white_alpha(28)),
    );

    let stroke = if flashing {
        Stroke::new(2.0_f32, GREEN)
    } else if selected {
        Stroke::new(2.0_f32, CYAN)
    } else {
        Stroke::new(1.0_f32, Color32::from_rgb(49, 65, 90))
    };
    p.rect_stroke(r, 10.0, stroke, egui::StrokeKind::Inside);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub wash: Color32,
    pub tile: Color32,
    pub sidebar: Color32,
    pub card: Color32,
    pub hover: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub purple: Color32,
    pub cyan: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub border: Color32,
    pub text_secondary: Color32,
}

pub fn palette(ctx: &egui::Context) -> Palette {
    if ctx.style().visuals.dark_mode {
        let card = Color32::from_rgb(28, 42, 64);
        let border = Color32::from_rgb(49, 65, 90);
        Palette {
            bg: Color32::from_rgb(11, 18, 32),
            surface: Color32::from_rgb(20, 31, 50),
            ink: Color32::from_rgb(241, 245, 251),
            muted: Color32::from_rgb(117, 134, 159),
            line: border,
            wash: Color32::from_rgb(23, 59, 121),
            tile: card,
            sidebar: Color32::from_rgb(16, 26, 43),
            card,
            hover: Color32::from_rgb(41, 61, 91),
            accent: BLUE,
            accent_hover: ACCENT_HOVER,
            purple: PURPLE,
            cyan: CYAN,
            success: GREEN,
            warning: WARNING,
            danger: DANGER,
            border,
            text_secondary: Color32::from_rgb(167, 181, 204),
        }
    } else {
        let card = Color32::WHITE;
        let border = Color32::from_rgb(205, 215, 230);
        Palette {
            bg: Color32::from_rgb(241, 245, 251),
            surface: Color32::from_rgb(248, 250, 253),
            ink: Color32::from_rgb(15, 23, 42),
            muted: Color32::from_rgb(100, 116, 139),
            line: border,
            wash: Color32::from_rgb(214, 230, 255),
            tile: card,
            sidebar: Color32::from_rgb(232, 238, 247),
            card,
            hover: Color32::from_rgb(226, 235, 248),
            accent: BLUE,
            accent_hover: Color32::from_rgb(37, 99, 235),
            purple: Color32::from_rgb(124, 58, 237),
            cyan: Color32::from_rgb(8, 145, 178),
            success: Color32::from_rgb(16, 185, 129),
            warning: Color32::from_rgb(217, 119, 6),
            danger: Color32::from_rgb(225, 29, 72),
            border,
            text_secondary: Color32::from_rgb(51, 65, 85),
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
    style.visuals.faint_bg_color = p.sidebar;
    style.visuals.code_bg_color = p.card;
    style.visuals.override_text_color = Some(p.ink);
    style.visuals.hyperlink_color = p.accent_hover;
    style.visuals.warn_fg_color = p.warning;
    style.visuals.error_fg_color = p.danger;
    style.visuals.selection.bg_fill = p.wash;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, p.accent);
    style.visuals.window_corner_radius = egui::CornerRadius::same(14);
    style.visuals.window_stroke = Stroke::new(1.0_f32, p.border);
    style.visuals.menu_corner_radius = egui::CornerRadius::same(12);

    style.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(12);
    style.visuals.widgets.noninteractive.bg_fill = p.surface;
    style.visuals.widgets.noninteractive.weak_bg_fill = p.card;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.ink);

    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(8);
        widget.bg_stroke = Stroke::new(1.0_f32, p.line);
        widget.fg_stroke = Stroke::new(1.0_f32, p.ink);
    }

    style.visuals.widgets.inactive.bg_fill = p.card;
    style.visuals.widgets.inactive.weak_bg_fill = p.tile;

    style.visuals.widgets.hovered.bg_fill = p.hover;
    style.visuals.widgets.hovered.weak_bg_fill = p.hover;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent);

    style.visuals.widgets.active.bg_fill = p.wash;
    style.visuals.widgets.active.weak_bg_fill = p.wash;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, p.accent_hover);

    style.visuals.widgets.open.bg_fill = p.hover;
    style.visuals.widgets.open.weak_bg_fill = p.hover;
    style.visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, p.accent);

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
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
    ctx.set_style(style);
}

pub fn primary(text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    egui::Button::new(text)
        .fill(BLUE)
        .stroke(Stroke::NONE)
        .corner_radius(8)
}

#[allow(dead_code)] // used by the upcoming layout refresh
pub fn secondary_button(text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    egui::Button::new(text)
        .fill(Color32::from_rgb(28, 42, 64))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(49, 65, 90)))
        .corner_radius(8)
}

#[allow(dead_code)] // used by the upcoming layout refresh
pub fn danger_button(text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    egui::Button::new(text)
        .fill(DANGER)
        .stroke(Stroke::NONE)
        .corner_radius(8)
}

#[allow(dead_code)] // used by the upcoming layout refresh
pub fn card_frame(p: Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(12)
        .inner_margin(egui::Margin::symmetric(12, 12))
}

#[allow(dead_code)] // used by the upcoming layout refresh
pub fn panel_frame(p: Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(14)
        .inner_margin(egui::Margin::symmetric(16, 16))
}

#[allow(dead_code)] // used by the upcoming layout refresh
pub fn section_title(ui: &mut egui::Ui, text: &str) {
    let p = palette(ui.ctx());
    ui.label(egui::RichText::new(text).size(17.0).strong().color(p.ink));
}

pub fn pill(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.14))
        .stroke(Stroke::new(1.0_f32, color.gamma_multiply(0.32)))
        .corner_radius(20)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).color(color).size(11.0).strong());
        });
}

#[allow(dead_code)] // used by the upcoming layout refresh
pub fn status_badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.14))
        .stroke(Stroke::new(1.0_f32, color.gamma_multiply(0.32)))
        .corner_radius(20)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                ui.painter().circle_filled(dot.center(), 3.5, color);
                ui.label(egui::RichText::new(text).color(color).size(11.0).strong());
            });
        });
}

pub fn toggle(ui: &mut egui::Ui, value: &mut bool) -> egui::Response {
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(46.0, 26.0), egui::Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    let p = palette(ui.ctx());
    let t = ui.ctx().animate_bool(response.id, *value);
    let off_bg = if response.hovered() { p.hover } else { p.border };
    let on_bg = if response.hovered() {
        p.accent_hover
    } else {
        p.accent
    };
    let bg = Color32::from_rgb(
        egui::lerp((off_bg.r() as f32)..=(on_bg.r() as f32), t) as u8,
        egui::lerp((off_bg.g() as f32)..=(on_bg.g() as f32), t) as u8,
        egui::lerp((off_bg.b() as f32)..=(on_bg.b() as f32), t) as u8,
    );
    ui.painter().rect_filled(rect, 13.0, bg);
    let center = egui::pos2(
        egui::lerp((rect.left() + 13.0)..=(rect.right() - 13.0), t),
        rect.center().y,
    );
    ui.painter()
        .circle_filled(center, 10.0, Color32::from_rgb(241, 245, 251));
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
        9..=11 => {
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
