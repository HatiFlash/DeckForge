//! Châssis SVG 3D sans art individuel des touches ; relief et contenus dynamiques par egui.
//!
//! Trois couches :
//! 1. ce châssis (`res/deck/chassis.svg`, sans les faces des touches), rastérisé une fois par
//!    taille en pixels physiques puis gardé en texture ;
//! 2. les faces des touches, dessinées par egui exactement dans les emplacements du SVG
//!    (sélection, survol, glisser-déposer, flash d'appui) ;
//! 3. le contenu des touches (aperçu LCD : icônes, images, GIF, vidéos, titres), inchangé.
use eframe::egui::{self, Color32, Rect, Stroke, Vec2};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

const SVG: &str = include_str!("../res/deck/chassis.svg");
/// viewBox du SVG, recadrée sur le boîtier et son ombre (coordonnées d'origine 1400 × 860).
const ORIGIN: Vec2 = Vec2::new(22.0, 14.0);
pub const VIEW: Vec2 = Vec2::new(1356.0, 838.0);
pub const COLS: usize = 5;
pub const ROWS: usize = 3;
/// Le rectangle intérieur du SVG est la référence commune au châssis et aux 15 touches.
const INNER_MIN: Vec2 = Vec2::new(65.0, 54.0);
const INNER_SIZE: Vec2 = Vec2::new(1270.0, 730.0);
const GRID_PADDING: Vec2 = Vec2::new(25.0, 26.0);
const GRID_GAP: Vec2 = Vec2::new(20.0, 18.0);
pub const FACE_SIZE: Vec2 = Vec2::new(
    (INNER_SIZE.x - 2.0 * GRID_PADDING.x - 4.0 * GRID_GAP.x) / COLS as f32,
    (INNER_SIZE.y - 2.0 * GRID_PADDING.y - 2.0 * GRID_GAP.y) / ROWS as f32,
);
const FACE_ORIGIN: Vec2 = Vec2::new(INNER_MIN.x + GRID_PADDING.x, INNER_MIN.y + GRID_PADDING.y);
const FACE_PITCH: Vec2 = Vec2::new(FACE_SIZE.x + GRID_GAP.x, FACE_SIZE.y + GRID_GAP.y);
const FACE_RADIUS: f32 = 18.0;

/// Le châssis SVG ne sert qu'au format 5 × 3 (FIFINE D6 et équivalents).
pub fn applies(cols: usize, rows: usize) -> bool {
    cols == COLS && rows == ROWS
}

/// Plus grand rectangle sans déformation, centré dans la zone disponible.
pub fn fit(available: Rect) -> Rect {
    let scale = (available.width() / VIEW.x).min(available.height() / VIEW.y).max(0.05);
    let size = VIEW * scale;
    Rect::from_center_size(available.center(), size)
}

/// Échelle du châssis (points d'écran par unité SVG).
pub fn scale(chassis: Rect) -> f32 {
    chassis.width() / VIEW.x
}

/// Convertit une position SVG absolue en points d'écran avec le même repère que les touches.
pub fn svg_point(chassis: Rect, point: egui::Pos2) -> egui::Pos2 {
    chassis.min + (point.to_vec2() - ORIGIN) * scale(chassis)
}

/// Face de la touche `index` (0 à 14) dans le repère commun du SVG.
pub fn face(chassis: Rect, index: usize) -> Rect {
    let (col, row) = ((index % COLS) as f32, (index / COLS) as f32);
    let min = svg_point(chassis, (FACE_ORIGIN + Vec2::new(FACE_PITCH.x * col, FACE_PITCH.y * row)).to_pos2());
    Rect::from_min_size(min, FACE_SIZE * scale(chassis))
}

/// Zone carrée de l'aperçu LCD, centrée dans la face (le contenu n'est jamais déformé).
pub fn lcd(face: Rect, scale: f32) -> Rect {
    let side = (face.width().min(face.height()) - 16.0 * scale).max(8.0);
    Rect::from_center_size(face.center(), Vec2::splat(side))
}

/// Rayon de l'écran d'une touche : concentrique à l'arrondi de la face (même centre de courbure).
pub fn lcd_radius(face: Rect, scale: f32) -> f32 {
    let inset = (face.height() - lcd(face, scale).height()) * 0.5;
    (FACE_RADIUS * scale - inset).max(4.0 * scale)
}

fn rasterize(width_px: u32) -> Option<Arc<egui::ColorImage>> {
    static CACHE: OnceLock<Mutex<HashMap<u32, Arc<egui::ColorImage>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(found) = cache.lock().ok()?.get(&width_px) {
        return Some(found.clone());
    }
    let mut options = resvg::usvg::Options::default();
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    options.image_href_resolver.resolve_data = Box::new(|_, _, _| None);
    let tree = resvg::usvg::Tree::from_str(SVG, &options).ok()?;
    let height_px = (width_px as f32 * VIEW.y / VIEW.x).round() as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width_px, height_px)?;
    let scale = width_px as f32 / VIEW.x;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let image = Arc::new(egui::ColorImage::from_rgba_premultiplied([width_px as usize, height_px as usize], pixmap.data()));
    let mut guard = cache.lock().ok()?;
    if guard.len() >= 6 {
        guard.clear();
    }
    guard.insert(width_px, image.clone());
    Some(image)
}

/// Largeur en pixels physiques arrondie par paliers de 64 : redimensionner la fenêtre ne
/// rastérise pas le SVG à chaque image, et le rendu reste net (au plus 64 px de marge).
pub fn bucket(width_points: f32, pixels_per_point: f32) -> u32 {
    let px = (width_points * pixels_per_point).ceil().clamp(128.0, 4096.0) as u32;
    px.div_ceil(64) * 64
}

/// Nombre de rastérisations du châssis pour ce contexte (contrôle de non-régression).
#[cfg(test)]
pub fn textures_loaded(ctx: &egui::Context) -> usize {
    ctx.data(|d| d.get_temp::<Arc<Mutex<HashMap<u32, egui::TextureHandle>>>>(egui::Id::new("deck_chassis")))
        .and_then(|m| m.lock().ok().map(|m| m.len()))
        .unwrap_or(0)
}

/// Peint le châssis dans `chassis`. Retourne faux si le SVG n'a pas pu être rendu (repli).
pub fn paint(painter: &egui::Painter, chassis: Rect) -> bool {
    let ctx = painter.ctx();
    // Jamais au-delà de la taille de texture du GPU (le châssis est alors légèrement agrandi).
    let max_side = ctx.input(|i| i.max_texture_side).max(256) as f32 * VIEW.x / VIEW.y.max(VIEW.x);
    let px = bucket(chassis.width(), ctx.pixels_per_point()).min((max_side as u32 / 64) * 64);
    let textures = ctx.data_mut(|d| d.get_temp_mut_or_default::<Arc<Mutex<HashMap<u32, egui::TextureHandle>>>>(egui::Id::new("deck_chassis")).clone());
    let Ok(mut textures) = textures.lock() else { return false };
    let texture = match textures.get(&px) {
        Some(texture) => texture.clone(),
        None => {
            let Some(image) = rasterize(px) else { return false };
            if textures.len() >= 4 {
                textures.clear();
            }
            let texture = ctx.load_texture(format!("deck-chassis-{px}"), (*image).clone(), egui::TextureOptions::LINEAR);
            textures.insert(px, texture.clone());
            texture
        }
    };
    painter.image(texture.id(), chassis, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
    // Neutral case above the cached geometry: same SVG coordinate system, no blue rim.
    let p = crate::theme::palette(ctx);
    let s = scale(chassis);
    let outer = Rect::from_min_max(svg_point(chassis, egui::pos2(31.0, 20.0)), svg_point(chassis, egui::pos2(1369.0, 818.0)));
    for (spread, opacity) in [(12.0, 9), (7.0, 13), (3.0, 17)] {
        painter.rect_filled(outer.translate(Vec2::new(0.0, 8.0 * s)).expand(spread * s), 80.0 * s, Color32::from_black_alpha(opacity));
    }
    let shell_top = if ctx.style().visuals.dark_mode { Color32::from_rgb(43, 48, 56) } else { Color32::from_rgb(57, 61, 67) };
    crate::theme::rounded_gradient(painter, outer, 80.0 * s, shell_top, Color32::from_rgb(22, 25, 30));
    painter.rect_stroke(outer, 80.0 * s, Stroke::new((1.0 * s).max(0.8), p.border_strong), egui::StrokeKind::Inside);
    let inset = Rect::from_min_max(svg_point(chassis, egui::pos2(65.0, 54.0)), svg_point(chassis, egui::pos2(1335.0, 784.0)));
    painter.rect_filled(inset, 63.0 * s, Color32::from_rgb(18, 21, 26));
    painter.line_segment([svg_point(chassis, egui::pos2(108.0, 60.0)), svg_point(chassis, egui::pos2(1290.0, 60.0))], Stroke::new((1.0 * s).max(0.5), Color32::from_white_alpha(32)));
    true
}

/// Face 3D dynamique : ombre intérieure dans le dégradé, un unique contour et un reflet discret.
pub fn paint_face(painter: &egui::Painter, face: Rect, scale: f32, selected: bool, hovering: bool, flashing: bool) {
    let p = crate::theme::palette(painter.ctx());
    let radius = 10.0;
    let glow = if flashing { crate::theme::GREEN } else { p.accent_ring };
    if selected || flashing {
        painter.rect_filled(face.expand(2.0), radius + 2.0, glow.gamma_multiply(0.14));
    }
    let fade = painter.ctx().animate_bool_with_time(egui::Id::new(("deck_face_hover", face.min.x.to_bits(), face.min.y.to_bits())), hovering, painter.ctx().style().animation_time.min(0.1));
    let top = if flashing { Color32::from_rgb(22, 68, 62) } else {
        let blend = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * fade).round() as u8;
        Color32::from_rgb(blend(p.raised.r(), p.hover.r()), blend(p.raised.g(), p.hover.g()), blend(p.raised.b(), p.hover.b()))
    };
    let bottom = if flashing { Color32::from_rgb(15, 46, 44) } else { p.sunken };
    crate::theme::rounded_gradient(painter, face, radius, top, bottom);
    let stroke = if flashing { crate::theme::GREEN } else if selected { p.accent_ring } else { p.hairline };
    painter.rect_stroke(face, radius, Stroke::new(if selected || flashing { 2.0 } else { 1.0 }, stroke), egui::StrokeKind::Inside);
    if painter.ctx().style().visuals.dark_mode {
        painter.line_segment([
            face.left_top() + Vec2::new(20.0, 2.0) * scale,
            face.right_top() + Vec2::new(-20.0, 2.0) * scale,
        ], Stroke::new(1.0, Color32::from_white_alpha(25)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chassis_contains_no_key_art_and_dynamic_faces_fill_the_inner_shell() {
        assert!(!SVG.contains("id=\"key-"), "aucune bordure de touche dans le SVG");
        assert!(!SVG.contains("url(#edge)"), "aucun biseau individuel dans le SVG");
        assert!(!SVG.contains("key-shadow"), "aucune ombre individuelle dans le SVG");
        let chassis = Rect::from_min_size(ORIGIN.to_pos2(), VIEW);
        let first = face(chassis, 0);
        let last = face(chassis, 14);
        assert_eq!(first.min, egui::pos2(90.0, 80.0));
        assert_eq!(first.size(), Vec2::new(228.0, 214.0));
        assert_eq!(last.max, egui::pos2(1310.0, 758.0));
        assert_eq!((face(chassis, 1).left() - first.right()).round(), 20.0);
        assert_eq!((face(chassis, 5).top() - first.bottom()).round(), 18.0);
        for available in [Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(900.0,300.0)), Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(500.0,900.0))] {
            let fitted = fit(available);
            assert!(available.contains_rect(fitted));
            assert!((fitted.width()/fitted.height()-VIEW.x/VIEW.y).abs()<1e-3);
            assert!(fitted.contains_rect(face(fitted,0)) && fitted.contains_rect(face(fitted,14)));
        }
    }

    #[test]
    fn dynamic_faces_remain_in_the_inner_shell_at_every_scale() {
        let chassis = Rect::from_min_size(ORIGIN.to_pos2(), VIEW);
        let inner = Rect::from_min_size(INNER_MIN.to_pos2(), INNER_SIZE);
        for i in 0..15 {
            assert!(inner.contains_rect(face(chassis, i)), "touche {i} contenue dans le châssis");
        }
        assert_eq!(face(chassis, 0).left()-inner.left(), GRID_PADDING.x);
        assert_eq!(inner.right()-face(chassis, 4).right(), GRID_PADDING.x);
        assert_eq!(inner.bottom()-face(chassis, 14).bottom(), GRID_PADDING.y);
        for available in [Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(900.0, 300.0)), Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(500.0, 900.0))] {
            let c = fit(available);
            assert!((c.width() / c.height() - VIEW.x / VIEW.y).abs() < 1e-3, "sans déformation");
            assert!(available.contains_rect(c));
            let s = scale(c);
            for i in 0..15 {
                let f = face(c, i);
                assert!(c.contains_rect(f));
                let l = lcd(f, s);
                assert!((l.width() - l.height()).abs() < 1e-3 && f.contains_rect(l), "aperçu carré dans la face");
            }
        }
    }

    #[test]
    fn chassis_is_rasterized_once_per_size_bucket() {
        assert_eq!(bucket(700.0, 1.0), 704);
        assert_eq!(bucket(700.0, 1.5), 1088);
        assert!(SVG.contains(r#"viewBox="22 14 1356 838""#));
        assert_eq!(bucket(690.0, 1.0), bucket(700.0, 1.0), "même palier : pas de nouveau rendu en redimensionnant");
        let image = rasterize(704).expect("SVG rendu");
        assert_eq!(image.size, [704, 435]);
        let opaque = image.pixels.iter().filter(|p| p.a() > 200).count();
        assert!(opaque > 704 * 435 / 2, "châssis visible");
        // Le centre de la touche reste le fond sombre du châssis, jamais une face SVG.
        let s = 704.0 / VIEW.x;
        let center = face(Rect::from_min_size(egui::Pos2::ZERO, VIEW * s), 7).center();
        let p = image.pixels[(center.y as usize) * 704 + center.x as usize];
        assert!(p.r() < 45 && p.b() < 90, "aucune surface de touche prédessinée : {p:?}");
        assert!(Arc::ptr_eq(&image, &rasterize(704).unwrap()));
    }
}
