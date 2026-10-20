//! Habillage SVG du pavé 5 × 3 : châssis, reliefs, ombres et emplacements des touches.
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
/// Face d'une touche dans la viewBox : origine, pas et taille.
const FACE_ORIGIN: Vec2 = Vec2::new(113.0, 104.0);
const FACE_PITCH: Vec2 = Vec2::new(244.0, 225.0);
pub const FACE_SIZE: Vec2 = Vec2::new(202.0, 190.0);
const FACE_RADIUS: f32 = 21.0;

/// Le châssis SVG ne sert qu'au format 5 × 3 (FIFINE D6 et équivalents).
pub fn applies(cols: usize, rows: usize) -> bool {
    cols == COLS && rows == ROWS
}

/// Plus grand rectangle aux proportions du SVG dans `available`, centré horizontalement, en haut.
pub fn fit(available: Rect) -> Rect {
    let scale = (available.width() / VIEW.x).min(available.height() / VIEW.y).max(0.05);
    let size = VIEW * scale;
    Rect::from_min_size(egui::pos2(available.center().x - size.x / 2.0, available.top()), size)
}

/// Échelle du châssis (points d'écran par unité SVG).
pub fn scale(chassis: Rect) -> f32 {
    chassis.width() / VIEW.x
}

/// Face de la touche `index` (0 à 14), alignée sur son emplacement du SVG.
pub fn face(chassis: Rect, index: usize) -> Rect {
    let s = scale(chassis);
    let (col, row) = ((index % COLS) as f32, (index / COLS) as f32);
    let min = chassis.min + (FACE_ORIGIN - ORIGIN + Vec2::new(FACE_PITCH.x * col, FACE_PITCH.y * row)) * s;
    Rect::from_min_size(min, FACE_SIZE * s)
}

/// Zone carrée de l'aperçu LCD, centrée dans la face (le contenu n'est jamais déformé).
pub fn lcd(face: Rect, scale: f32) -> Rect {
    let side = (face.width().min(face.height()) - 16.0 * scale).max(8.0);
    Rect::from_center_size(face.center(), Vec2::splat(side))
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
    true
}

/// Face dynamique d'une touche, dans le style du SVG (dégradé, liseré clair, reflets).
pub fn paint_face(painter: &egui::Painter, face: Rect, scale: f32, selected: bool, flashing: bool) {
    let radius = FACE_RADIUS * scale;
    let (top, bottom) = if flashing {
        (Color32::from_rgb(40, 104, 92), Color32::from_rgb(20, 60, 56))
    } else if selected {
        (Color32::from_rgb(52, 88, 150), Color32::from_rgb(28, 50, 96))
    } else {
        (Color32::from_rgb(64, 82, 110), Color32::from_rgb(32, 43, 64))
    };
    crate::theme::rounded_gradient(painter, face, radius, top, bottom);
    painter.rect_stroke(face, radius, Stroke::new((1.5 * scale).max(1.0), Color32::from_rgba_unmultiplied(139, 175, 221, 178)), egui::StrokeKind::Inside);
    let line = |a: Vec2, b: Vec2, alpha: u8| {
        painter.line_segment([face.min + a * scale, face.min + b * scale], Stroke::new((2.0 * scale).max(1.0), Color32::from_rgba_unmultiplied(197, 223, 255, alpha)));
    };
    line(Vec2::new(18.0, 4.0), Vec2::new(183.0, 4.0), 87);
    line(Vec2::new(9.0, 33.0), Vec2::new(9.0, 153.0), 46);
    if selected || flashing {
        let color = if flashing { crate::theme::GREEN } else { crate::theme::CYAN };
        painter.rect_stroke(face.expand(3.0 * scale + 1.0), radius + 3.0 * scale, Stroke::new((5.0 * scale).max(2.0), color), egui::StrokeKind::Outside);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_match_the_svg_slots_and_keep_proportions() {
        // Les emplacements du SVG (puits + biseau) entourent chaque face d'environ 11 unités.
        let chassis = Rect::from_min_size(ORIGIN.to_pos2(), VIEW);
        let slots = [(102.0, 93.0), (346.0, 93.0), (1078.0, 543.0)];
        for (i, (x, y)) in [0usize, 1, 14].into_iter().zip(slots) {
            let f = face(chassis, i);
            assert_eq!((f.min.x - x, f.min.y - y), (11.0, 11.0), "touche {i}");
            assert!(SVG.contains(&format!("x=\"{}\" y=\"{}\" width=\"224\" height=\"212\"", x as i32, y as i32)), "emplacement {i} présent");
        }
        assert!(!SVG.contains("url(#key)"), "faces pleines retirées du SVG");
        assert_eq!(SVG.matches("url(#edge)").count(), 15, "les 15 biseaux restent");
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
        // Centre d'une face : vide dans le SVG (biseau seulement), la face vient d'egui.
        let s = 704.0 / VIEW.x;
        let center = face(Rect::from_min_size(egui::Pos2::ZERO, VIEW * s), 7).center();
        let p = image.pixels[(center.y as usize) * 704 + center.x as usize];
        assert!(p.r() > 20 || p.b() > 60, "le biseau de l'emplacement reste visible sous la face : {p:?}");
        assert!(Arc::ptr_eq(&image, &rasterize(704).unwrap()));
    }
}
