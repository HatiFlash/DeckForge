use eframe::egui::{self, Color32, Stroke, Vec2};

pub const BLUE: Color32 = Color32::from_rgb(52, 120, 246);
pub const GREEN: Color32 = Color32::from_rgb(34, 197, 139);
pub const PURPLE: Color32 = Color32::from_rgb(139, 92, 246);
pub const CYAN: Color32 = Color32::from_rgb(34, 195, 238);
pub const WARNING: Color32 = Color32::from_rgb(245, 173, 66);
pub const DANGER: Color32 = Color32::from_rgb(242, 95, 112);

/// Jetons de mise en page NovyDeck (voir docs/DESIGN.md) : espacements et rayons partagés.
pub const SPACE: [f32; 6] = [4.0, 8.0, 12.0, 16.0, 24.0, 32.0];
/// Rayons : contrôle (bouton, champ), menu/carte, fenêtre/panneau.
pub const RADIUS_SM: u8 = 8;
pub const RADIUS_MD: u8 = 12;
pub const RADIUS_LG: u8 = 16;
pub const RADIUS_XS: u8 = 4;
pub const RADIUS_BUTTON: u8 = 6;
pub const BORDER_HAIRLINE: f32 = 1.0;
pub const MOTION_FAST: f32 = 0.12;
/// Écran d'une touche hors châssis SVG (face `key_face` de rayon 10, écran inséré de 5) :
/// les fonds et médias sont masqués à ce rayon, concentrique à la face.
pub const KEY_LCD_RADIUS: f32 = 6.0;

fn decode_icon(bytes: &[u8]) -> egui::IconData {
    match image::load_from_memory(bytes) {
        Ok(image) => {
            let rgba = image.to_rgba8();
            egui::IconData { width: rgba.width(), height: rgba.height(), rgba: rgba.into_raw() }
        }
        Err(_) => egui::IconData { rgba: vec![0; 32 * 32 * 4], width: 32, height: 32 },
    }
}
/// 32 × 32 logo (notification-area icon expects exactly this size).
pub fn app_icon() -> egui::IconData { decode_icon(include_bytes!("../res/branding/novydeck-32.png")) }
/// Larger logo for the window and taskbar.
pub fn window_icon() -> egui::IconData { decode_icon(include_bytes!("../res/branding/novydeck-64.png")) }
/// Header logo, uploaded once and kept by egui.
/// Logo net en grand (intro du démarrage), décodé une seule fois.
pub fn intro_logo_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::new("deckforge_intro_logo_texture");
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) { return texture; }
    let icon = decode_icon(include_bytes!("../res/branding/novydeck-256.png"));
    let image = egui::ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
    let texture = ctx.load_texture("deckforge-intro-logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    texture
}

pub fn logo_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::new("deckforge_logo_texture");
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) { return texture; }
    let icon = window_icon();
    let image = egui::ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
    let texture = ctx.load_texture("deckforge-logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    texture
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

/// Dessine une texture masquée par un rectangle arrondi (fond, image, image de GIF ou vidéo).
/// `clip_rect` d'egui est rectangulaire : le masque est donc la géométrie elle-même, un
/// éventail de triangles texturés dont les UV suivent la position. Une couronne d'un pixel
/// physique passe de l'opacité à la transparence pour lisser le bord sans halo coloré :
/// aucune texture ni masque n'est recalculé image après image.
pub fn rounded_image(p: &egui::Painter, texture: egui::TextureId, r: egui::Rect, radius: f32) {
    let radius = radius.clamp(0.0, r.width().min(r.height()) * 0.5);
    let feather = 1.0 / p.ctx().pixels_per_point();
    let uv = |pos: egui::Pos2| egui::pos2((pos.x - r.left()) / r.width(), (pos.y - r.top()) / r.height());
    let segments = ((radius * p.ctx().pixels_per_point()).sqrt().ceil() as usize).clamp(2, 12);
    let mut mesh = egui::Mesh::with_texture(texture);
    let inner = r.shrink(feather * 0.5);
    let corner_radius = (radius - feather * 0.5).max(0.0);
    mesh.vertices.push(egui::epaint::Vertex { pos: r.center(), uv: uv(r.center()), color: Color32::WHITE });
    let mut ring = Vec::new();
    for (center, start) in [
        (inner.left_top() + Vec2::splat(corner_radius), std::f32::consts::PI),
        (inner.right_top() + Vec2::new(-corner_radius, corner_radius), std::f32::consts::PI * 1.5),
        (inner.right_bottom() - Vec2::splat(corner_radius), 0.0),
        (inner.left_bottom() + Vec2::new(corner_radius, -corner_radius), std::f32::consts::FRAC_PI_2),
    ] {
        for i in 0..=segments {
            let dir = Vec2::angled(start + i as f32 * std::f32::consts::FRAC_PI_2 / segments as f32);
            ring.push((center + dir * corner_radius, dir));
        }
    }
    let n = ring.len() as u32;
    for &(pos, dir) in &ring {
        mesh.vertices.push(egui::epaint::Vertex { pos, uv: uv(pos), color: Color32::WHITE });
        let outer = pos + dir * feather;
        mesh.vertices.push(egui::epaint::Vertex { pos: outer, uv: uv(outer), color: Color32::TRANSPARENT });
    }
    for i in 0..n {
        let j = (i + 1) % n;
        let (a, a_out, b, b_out) = (1 + 2 * i, 2 + 2 * i, 1 + 2 * j, 2 + 2 * j);
        mesh.add_triangle(0, a, b);
        mesh.add_triangle(a, a_out, b_out);
        mesh.add_triangle(a, b_out, b);
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
    pub surface_1: Color32,
    pub surface_2: Color32,
    pub surface_3: Color32,
    pub border_subtle: Color32,
    pub border_strong: Color32,
    pub text_primary: Color32,
    pub text_tertiary: Color32,
    pub accent_active: Color32,
    pub accent_subtle: Color32,
    pub success_subtle: Color32,
    pub warning_subtle: Color32,
    pub danger_subtle: Color32,
    /// Contrasting ink for filled accent buttons, chosen separately for each accent.
    pub on_accent: Color32,
}

/// Styles proposés dans Réglages → Thème. `dark` indique le mode egui associé.
pub const SCHEMES: [(&str, &str, bool); 4] = [
    ("navy", "Bleu nuit", true),
    ("graphite", "Graphite", true),
    ("clair", "Clair", false),
    ("sable", "Sable", false),
];
/// Couleurs d'accent (boutons principaux, sélection, onglets actifs).
pub const ACCENTS: [(&str, &str, Color32); 5] = [
    ("bleu", "Bleu", BLUE),
    ("violet", "Violet", PURPLE),
    ("emeraude", "Émeraude", Color32::from_rgb(16, 163, 127)),
    ("orange", "Orange", Color32::from_rgb(232, 110, 34)),
    ("rose", "Rose", Color32::from_rgb(219, 39, 119)),
];
/// Translate only the display label; stored scheme and accent identifiers stay stable.
pub fn display_color_name(name: &str) -> &str {
    match name {
        "Bleu nuit" => crate::i18n::tr("Bleu nuit"),
        "Graphite" => crate::i18n::tr("Graphite"),
        "Clair" => crate::i18n::tr("Clair"),
        "Sable" => crate::i18n::tr("Sable"),
        "Bleu" => crate::i18n::tr("Bleu"),
        "Violet" => crate::i18n::tr("Violet"),
        "Émeraude" => crate::i18n::tr("Émeraude"),
        "Orange" => crate::i18n::tr("Orange"),
        "Rose" => crate::i18n::tr("Rose"),
        _ => name,
    }
}
static DARK_SCHEME: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static LIGHT_SCHEME: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(2);
static ACCENT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static DARK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Mémorise le style et l'accent choisis (identifiants inconnus = valeurs par défaut,
/// pour rester compatible avec les anciens preferences.json).
pub fn set_theme(scheme: &str, accent: &str) {
    use std::sync::atomic::Ordering::Relaxed;
    if let Some(i) = SCHEMES.iter().position(|s| s.0 == scheme) {
        if SCHEMES[i].2 { DARK_SCHEME.store(i, Relaxed) } else { LIGHT_SCHEME.store(i, Relaxed) }
    }
    ACCENT.store(ACCENTS.iter().position(|a| a.0 == accent).unwrap_or(0), Relaxed);
}
/// Couleur d'accent courante.
pub fn accent() -> Color32 {
    ACCENTS[ACCENT.load(std::sync::atomic::Ordering::Relaxed).min(ACCENTS.len() - 1)].2
}
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}
fn luminance(c: Color32) -> f32 {
    let channel = |v: u8| { let v = v as f32 / 255.0; if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) } };
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}
fn contrast(a: Color32, b: Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
/// Palette pure d'un style et d'un accent (testable sans contexte egui).
pub fn palette_for(scheme: &str, accent: Color32) -> Palette {
    let rgb = Color32::from_rgb;
    let (bg, surface, card, sidebar, hover, border, ink, muted, secondary, dark) = match scheme {
        "graphite" => (rgb(17, 19, 25), rgb(26, 29, 37), rgb(35, 39, 49), rgb(21, 24, 31), rgb(46, 52, 65), rgb(66, 73, 88), rgb(241, 243, 248), rgb(170, 178, 191), rgb(201, 207, 218), true),
        "clair" => (rgb(248, 247, 244), rgb(255, 255, 255), rgb(255, 255, 255), rgb(243, 242, 238), rgb(237, 239, 243), rgb(202, 207, 215), rgb(27, 32, 42), rgb(88, 95, 107), rgb(57, 66, 79), false),
        "sable" => (rgb(246, 242, 235), rgb(253, 251, 247), rgb(255, 255, 255), rgb(238, 232, 221), rgb(236, 228, 214), rgb(207, 197, 179), rgb(41, 33, 25), rgb(99, 85, 70), rgb(74, 63, 51), false),
        _ => (rgb(13, 17, 26), rgb(22, 28, 40), rgb(31, 39, 54), rgb(17, 23, 34), rgb(43, 54, 72), rgb(62, 76, 97), rgb(241, 245, 251), rgb(168, 181, 199), rgb(201, 211, 224), true),
    };
    Palette {
        bg,
        surface,
        ink,
        muted,
        line: border,
        wash: if dark { mix(card, accent, 0.45) } else { mix(Color32::WHITE, accent, 0.16) },
        tile: card,
        sidebar,
        card,
        hover,
        accent,
        accent_hover: if dark { mix(accent, Color32::WHITE, 0.18) } else { mix(accent, Color32::BLACK, 0.15) },
        purple: if dark { PURPLE } else { rgb(124, 58, 237) },
        cyan: if dark { CYAN } else { rgb(8, 145, 178) },
        success: if dark { GREEN } else { rgb(5, 150, 105) },
        warning: if dark { WARNING } else { rgb(194, 105, 6) },
        danger: if dark { DANGER } else { rgb(205, 30, 70) },
        border,
        text_secondary: secondary,
        surface_1: surface,
        surface_2: card,
        surface_3: hover,
        border_subtle: border,
        border_strong: if dark { rgb(113, 130, 153) } else { rgb(105, 112, 124) },
        text_primary: ink,
        text_tertiary: muted,
        accent_active: if dark { mix(accent, Color32::WHITE, 0.3) } else { mix(accent, Color32::BLACK, 0.28) },
        accent_subtle: if dark { mix(card, accent, 0.20) } else { mix(card, accent, 0.12) },
        success_subtle: mix(card, if dark { GREEN } else { rgb(5, 112, 78) }, 0.16),
        warning_subtle: mix(card, if dark { WARNING } else { rgb(159, 82, 8) }, 0.16),
        danger_subtle: mix(card, if dark { DANGER } else { rgb(180, 31, 61) }, 0.16),
        on_accent: if contrast(Color32::WHITE, accent) >= 4.5 { Color32::WHITE } else { Color32::BLACK },
    }
}
pub fn current_scheme(dark: bool) -> &'static str {
    use std::sync::atomic::Ordering::Relaxed;
    SCHEMES[if dark { DARK_SCHEME.load(Relaxed) } else { LIGHT_SCHEME.load(Relaxed) }.min(SCHEMES.len() - 1)].0
}
fn current_palette() -> Palette {
    palette_for(current_scheme(DARK.load(std::sync::atomic::Ordering::Relaxed)), accent())
}

pub fn palette(ctx: &egui::Context) -> Palette {
    palette_for(current_scheme(ctx.style().visuals.dark_mode), accent())
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
    DARK.store(dark, std::sync::atomic::Ordering::Relaxed);
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
    style.visuals.selection.bg_fill = p.accent_subtle;
    style.visuals.selection.stroke = Stroke::new(2.0_f32, p.accent_active);
    style.visuals.window_shadow = egui::epaint::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(if dark { 110 } else { 36 }) };
    style.visuals.popup_shadow = egui::epaint::Shadow { offset: [0, 5], blur: 16, spread: 0, color: Color32::from_black_alpha(if dark { 110 } else { 35 }) };
    style.visuals.window_corner_radius = egui::CornerRadius::same(RADIUS_LG);
    style.visuals.window_stroke = Stroke::new(1.0_f32, p.border);
    style.visuals.menu_corner_radius = egui::CornerRadius::same(RADIUS_MD);

    style.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(RADIUS_MD);
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
        widget.corner_radius = egui::CornerRadius::same(RADIUS_SM);
        widget.bg_stroke = Stroke::new(1.0_f32, p.line);
        widget.fg_stroke = Stroke::new(1.0_f32, p.ink);
    }

    style.visuals.widgets.inactive.bg_fill = p.card;
    style.visuals.widgets.inactive.weak_bg_fill = p.surface_2;
    style.visuals.widgets.inactive.expansion = 0.0;
    style.visuals.widgets.hovered.expansion = 1.0;
    style.visuals.widgets.active.expansion = 0.0;
    style.visuals.widgets.open.expansion = 1.0;

    style.visuals.widgets.hovered.bg_fill = p.hover;
    style.visuals.widgets.hovered.weak_bg_fill = p.hover;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent);

    style.visuals.widgets.active.bg_fill = p.wash;
    style.visuals.widgets.active.weak_bg_fill = p.wash;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, p.accent_hover);

    style.visuals.widgets.open.bg_fill = p.hover;
    style.visuals.widgets.open.weak_bg_fill = p.hover;
    style.visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, p.accent);

    style.spacing.item_spacing = Vec2::splat(SPACE[1]);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.spacing.interact_size.y = 32.0;
    style.animation_time = MOTION_FAST;
    style.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.0));
    style.text_styles.insert(egui::TextStyle::Name("title".into()), egui::FontId::proportional(24.0));
    style.text_styles.insert(egui::TextStyle::Name("caption".into()), egui::FontId::proportional(12.0));
    style.text_styles.insert(egui::TextStyle::Name("mono".into()), egui::FontId::monospace(13.0));

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
    let p = current_palette();
    egui::Button::new(text.into().color(p.on_accent))
        .fill(p.accent)
        .stroke(Stroke::NONE)
        .corner_radius(RADIUS_BUTTON)
}

#[allow(dead_code)] // used by the upcoming layout refresh
pub fn secondary_button(text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    // Couleurs du thème courant : en clair, ces boutons restaient bleu nuit avec un texte sombre.
    let p = current_palette();
    egui::Button::new(text)
        .fill(p.card)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(8)
}

#[allow(dead_code)] // used by the upcoming layout refresh
pub fn danger_button(text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    egui::Button::new(text)
        .fill(current_palette().danger)
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

#[cfg(test)]
mod theme_tests {
    #[test]
    fn branding_assets_keep_transparency_and_visible_pixels() {
        for bytes in [
            include_bytes!("../res/branding/novydeck-logo.png").as_slice(),
            include_bytes!("../res/branding/novydeck-32.png").as_slice(),
            include_bytes!("../res/branding/novydeck-64.png").as_slice(),
            include_bytes!("../res/branding/novydeck-256.png").as_slice(),
        ] {
            let image = image::load_from_memory(bytes).unwrap().to_rgba8();
            for (x, y) in [(0, 0), (image.width() - 1, 0), (0, image.height() - 1), (image.width() - 1, image.height() - 1)] {
                assert_eq!(image.get_pixel(x, y).0[3], 0, "logo corner must be transparent");
            }
            assert!(image.pixels().any(|pixel| pixel.0[3] > 0 && pixel.0[3] < 255), "logo must retain semi-transparent pixels");
        }
        let icon = super::window_icon();
        assert_eq!(icon.rgba[3], 0, "window icon corner must be transparent");
    }

    #[test]
    fn layered_palette_meets_text_and_component_contrast_for_every_accent() {
        for (scheme, _, _) in super::SCHEMES {
            for (_, _, accent) in super::ACCENTS {
                let p = super::palette_for(scheme, accent);
                for surface in [p.bg, p.surface_1, p.surface_2, p.surface_3, p.accent_subtle] {
                    assert!(super::contrast(p.text_primary, surface) >= 4.5, "primary {scheme} {accent:?} {surface:?}");
                    assert!(super::contrast(p.text_secondary, surface) >= 4.5, "secondary {scheme} {accent:?} {surface:?}");
                    assert!(super::contrast(p.text_tertiary, surface) >= 4.5, "tertiary {scheme} {accent:?} {surface:?}");
                }
                assert!(super::contrast(p.on_accent, p.accent) >= 4.5, "button {scheme} {accent:?}");
                assert!(super::contrast(p.border_strong, p.surface_2) >= 3.0, "component {scheme}");
                for (foreground, background) in [(p.success, p.success_subtle), (p.warning, p.warning_subtle), (p.danger, p.danger_subtle)] {
                    assert!(super::contrast(foreground, background) >= 4.5, "badge {scheme} {foreground:?}");
                }
            }
        }
    }

    #[test]
    fn every_scheme_and_accent_keeps_readable_text_and_buttons() {
        fn lum(c: super::Color32) -> f32 {
            let f = |v: u8| { let v = v as f32 / 255.0; if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) } };
            0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
        }
        fn contrast(a: super::Color32, b: super::Color32) -> f32 {
            let (x, y) = (lum(a), lum(b));
            (x.max(y) + 0.05) / (x.min(y) + 0.05)
        }
        for (scheme, _, _) in super::SCHEMES {
            for (_, _, accent) in super::ACCENTS {
                let p = super::palette_for(scheme, accent);
                assert!(contrast(p.ink, p.bg) >= 7.0, "{scheme}: texte/fond");
                assert!(contrast(p.ink, p.card) >= 7.0, "{scheme}: texte/carte");
                assert!(contrast(p.muted, p.card) >= 3.0, "{scheme}: texte secondaire");
                assert!(contrast(super::Color32::WHITE, accent) >= 3.0, "{scheme}: bouton principal {accent:?}");
            }
        }
    }
}

/// Fenêtre de réglages intégrée : centrée sur l'application, ni déplaçable ni repliable.
pub fn fixed_window<'a>(title: impl Into<egui::WidgetText>) -> egui::Window<'a> {
    egui::Window::new(title)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .collapsible(false)
        .movable(false)
        .max_width(920.0)
        .max_height(620.0)
}
