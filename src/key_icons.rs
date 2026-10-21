//! Icônes par défaut des TOUCHES : écran LCD du périphérique et aperçu de la grille.
//!
//! Distinct de `iconset` (icônes de l'interface du PC) : même jeu de pictogrammes IconPark
//! embarqués, mais palette « écran » (contrastes forts, fond noir), pastille de couleur par
//! catégorie et composition dans l'image RVB envoyée au périphérique.
//!
//! Utilisées seulement quand la touche n'a ni image, ni symbole choisi par l'utilisateur
//! (`display_kind`) : les personnalisations ne changent jamais.
use crate::iconset::{Family, Ico};
use crate::model::Key;
use image::RgbImage;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Icône par défaut d'une touche, ou `None` si l'utilisateur a choisi un visuel.
pub fn for_key(key: &Key) -> Option<Ico> {
    if !key.image.is_empty() || key.display_kind.is_some() || key.steps.is_empty() {
        return None;
    }
    Some(crate::iconset::for_steps(&key.steps))
}

/// Couleur de catégorie (pastille) et couleurs IconPark adaptées à un petit écran sombre.
pub fn palette(family: Family) -> ([u8; 3], [[u8; 3]; 4]) {
    let (fill, inner): ([u8; 3], [u8; 3]) = match family {
        Family::Input => ([0xF0, 0x8C, 0x3C], [0xFF, 0xD5, 0x8F]),
        Family::App => ([0x3A, 0x82, 0xFF], [0x9C, 0xCB, 0xFF]),
        Family::Web => ([0x1F, 0x9B, 0xE0], [0x6B, 0xE8, 0xD8]),
        Family::Logic => ([0x86, 0x6B, 0xF2], [0x4F, 0xE0, 0xD0]),
        Family::Media => ([0xB0, 0x52, 0xF0], [0xFF, 0x86, 0xC0]),
        Family::Audio => ([0x18, 0xA8, 0xE6], [0x86, 0xEA, 0xFF]),
        Family::System => ([0x6A, 0x8A, 0xAE], [0xB8, 0xD2, 0xEC]),
        Family::Streaming => ([0xF0, 0x4E, 0x55], [0xFF, 0xA8, 0x82]),
        Family::Plugin => ([0x26, 0xB8, 0x76], [0x74, 0xCC, 0xFF]),
        Family::Ai => ([0x76, 0x66, 0xF0], [0x80, 0xC0, 0xFF]),
        Family::Image => ([0xF5, 0x5E, 0x84], [0xFF, 0xC8, 0x66]),
        Family::Warning => ([0xF5, 0xAD, 0x42], [0xFF, 0xE2, 0xA8]),
        Family::Danger => ([0xF2, 0x5F, 0x70], [0xFF, 0xC2, 0xC9]),
        // Navigation (pages, dossiers, profils) : bleu NovyDeck.
        Family::Accent | Family::Mono => ([0x3C, 0x80, 0xF6], [0x9E, 0xC6, 0xFF]),
    };
    let line = [mix(fill[0], 255, 0.70), mix(fill[1], 255, 0.70), mix(fill[2], 255, 0.70)];
    (fill, [line, fill, [255, 255, 255], inner])
}

fn mix(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

/// RVBA prémultiplié d'une icône de `px` pixels, rastérisée une seule fois par taille.
fn raster(ico: Ico, px: u32) -> Option<Arc<Vec<u8>>> {
    static CACHE: OnceLock<Mutex<HashMap<(Ico, u32), Option<Arc<Vec<u8>>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(found) = cache.lock().ok()?.get(&(ico, px)) {
        return found.clone();
    }
    let colors = palette(ico.family()).1.map(|[r, g, b]| eframe::egui::Color32::from_rgb(r, g, b));
    let pixels = crate::iconset::rasterize(ico, colors, px).map(|image| {
        Arc::new(image.pixels.iter().flat_map(|c| c.to_array()).collect::<Vec<u8>>())
    });
    let mut guard = cache.lock().ok()?;
    if guard.len() > 256 {
        guard.clear();
    }
    guard.insert((ico, px), pixels.clone());
    pixels
}

fn blend(image: &mut RgbImage, x: i64, y: i64, rgba: [u8; 4], premultiplied: bool) {
    if x < 0 || y < 0 || x >= image.width() as i64 || y >= image.height() as i64 || rgba[3] == 0 {
        return;
    }
    let target = image.get_pixel_mut(x as u32, y as u32);
    let a = rgba[3] as u32;
    for c in 0..3 {
        let source = if premultiplied { rgba[c] as u32 * 255 } else { rgba[c] as u32 * a };
        target[c] = ((source + target[c] as u32 * (255 - a) + 127) / 255).min(255) as u8;
    }
}

/// Pastille arrondie de la couleur de catégorie, puis l'icône, centrées sur `center`.
/// `tile` : pastille seulement sur fond noir (sans fond choisi), pour ne pas lutter avec la couleur de l'utilisateur.
pub fn draw(image: &mut RgbImage, ico: Ico, center: (f32, f32), side: u32, tile: bool) {
    let side = side.clamp(12, 256);
    if tile {
        let (fill, _) = palette(ico.family());
        let half = side as f32 * 0.66;
        let radius = half * 0.42;
        let (cx, cy) = center;
        for y in (cy - half).floor() as i64..=(cy + half).ceil() as i64 {
            for x in (cx - half).floor() as i64..=(cx + half).ceil() as i64 {
                // Distance à un carré arrondi, avec 1 px d'anticrénelage.
                let dx = ((x as f32 + 0.5 - cx).abs() - (half - radius)).max(0.0);
                let dy = ((y as f32 + 0.5 - cy).abs() - (half - radius)).max(0.0);
                let coverage = (radius - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    blend(image, x, y, [fill[0], fill[1], fill[2], (coverage * 0.30 * 255.0) as u8], false);
                }
            }
        }
    }
    let Some(pixels) = raster(ico, side) else { return };
    let x0 = (center.0 - side as f32 / 2.0).round() as i64;
    let y0 = (center.1 - side as f32 / 2.0).round() as i64;
    for y in 0..side {
        for x in 0..side {
            let i = ((y * side + x) * 4) as usize;
            blend(image, x0 + x as i64, y0 + y as i64, [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]], true);
        }
    }
}

/// Place l'icône d'une touche de `size` pixels : en haut si un titre suit, sinon centrée et plus grande.
pub fn layout(size: u32, scale: u8, has_label: bool) -> ((f32, f32), u32) {
    let factor = scale.clamp(20, 200) as f32 / 100.0;
    let s = size as f32;
    if has_label {
        ((s * 0.5, s * 0.31), (s * 0.38 * factor).round() as u32)
    } else {
        ((s * 0.5, s * 0.5), (s * 0.48 * factor).round() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Step;

    fn key(steps: Vec<Step>) -> Key {
        Key { steps, ..Default::default() }
    }

    #[test]
    fn default_icons_cover_native_actions_and_never_replace_user_choices() {
        let media = |c: &str| for_key(&key(vec![Step::Media { command: c.into() }]));
        assert_eq!(media("play_pause"), Some(Ico::Play));
        assert_eq!(media("stop"), Some(Ico::Stop));
        assert_eq!(media("volume_up"), Some(Ico::VolumeUp));
        assert_eq!(media("volume_down"), Some(Ico::VolumeDown));
        assert_eq!(media("mute"), Some(Ico::Mute));
        assert_eq!(for_key(&key(vec![Step::Launch { program: "x.exe".into(), args: vec![], focus_if_running: false }])), Some(Ico::Application));
        assert_eq!(for_key(&key(vec![Step::Hotkey { keys: "CTRL+C".into() }])), Some(Ico::Keyboard));
        assert_eq!(for_key(&key(vec![Step::PageNext])), Some(Ico::PageNext));
        assert_eq!(for_key(&key(vec![Step::PagePrevious])), Some(Ico::PagePrevious));
        assert_eq!(for_key(&key(vec![Step::Notify { title: String::new(), message: "x".into() }])), Some(Ico::Notify));
        assert_eq!(for_key(&key(vec![Step::Delay { ms: 1 }, Step::CloseWindow])), Some(Ico::MultiAction));
        let mut custom = key(vec![Step::Media { command: "mute".into() }]);
        custom.display_kind = Some(11);
        assert_eq!(for_key(&custom), None, "symbole choisi par l'utilisateur conservé");
        custom.display_kind = None;
        custom.image = "C:\\\\icone.png".into();
        assert_eq!(for_key(&custom), None, "image personnalisée conservée");
        assert_eq!(for_key(&Key::default()), None, "touche vide : pas d'icône");
        for kind in 0..crate::automation::NAMES.len() {
            let _ = for_key(&key(vec![crate::automation::step(kind)]));
        }
    }

    #[test]
    fn icons_are_centered_colourful_and_cached() {
        for size in [72u32, 96, 126] {
            let mut image = RgbImage::from_pixel(size, size, image::Rgb([0, 0, 0]));
            let (center, side) = layout(size, 100, false);
            draw(&mut image, Ico::VolumeUp, center, side, true);
            let lit: Vec<(u32, u32)> = image.enumerate_pixels().filter(|(_, _, p)| p.0.iter().any(|c| *c > 60)).map(|(x, y, _)| (x, y)).collect();
            let (min_x, max_x) = (lit.iter().map(|p| p.0).min().unwrap(), lit.iter().map(|p| p.0).max().unwrap());
            let (min_y, max_y) = (lit.iter().map(|p| p.1).min().unwrap(), lit.iter().map(|p| p.1).max().unwrap());
            assert!((min_x as i64 + max_x as i64 - size as i64 + 1).abs() <= 3 && (min_y as i64 + max_y as i64 - size as i64 + 1).abs() <= 3, "centrée à {size} px");
            assert!(min_x >= 2 && max_x + 2 < size, "marges à {size} px");
            let colours: std::collections::HashSet<[u8; 3]> = image.pixels().map(|p| [p[0] / 32, p[1] / 32, p[2] / 32]).collect();
            assert!(colours.len() >= 6, "plusieurs couleurs à {size} px");
        }
        let a = raster(Ico::Play, 40).unwrap();
        let b = raster(Ico::Play, 40).unwrap();
        assert!(Arc::ptr_eq(&a, &b), "rastérisée une seule fois");
    }

    /// Planche des icônes de touches : `DECKFORGE_UI_OUTPUT=dossier cargo test key_icons -- --ignored`.
    #[test]
    #[ignore = "écrit une planche PNG des icônes de touches"]
    fn key_icons_sheet() {
        let dir = std::env::var_os("DECKFORGE_UI_OUTPUT").expect("DECKFORGE_UI_OUTPUT");
        let samples: Vec<(&str, Vec<Step>)> = vec![
            ("Lecture", vec![Step::Media { command: "play_pause".into() }]),
            ("Stop", vec![Step::Media { command: "stop".into() }]),
            ("Volume +", vec![Step::Media { command: "volume_up".into() }]),
            ("Volume -", vec![Step::Media { command: "volume_down".into() }]),
            ("Muet", vec![Step::Media { command: "mute".into() }]),
            ("Appli", vec![Step::Launch { program: "x.exe".into(), args: vec![], focus_if_running: false }]),
            ("Raccourci", vec![Step::Hotkey { keys: "CTRL+C".into() }]),
            ("Souris", vec![crate::automation::step(27)]),
            ("OBS", vec![crate::automation::step(20)]),
            ("Dossier", vec![Step::Folder { id: "x".into() }]),
            ("Page +", vec![Step::PageNext]),
            ("Page -", vec![Step::PagePrevious]),
            ("Notif", vec![crate::automation::step(41)]),
            ("Variable", vec![crate::automation::step(16)]),
            ("Si…", vec![crate::automation::step(39)]),
            ("Site", vec![Step::Url { url: "https://x".into() }]),
        ];
        let font = crate::key_render::font().unwrap();
        let size = 126u32;
        let mut sheet = RgbImage::from_pixel(8 * (size + 8), 2 * (size + 8), image::Rgb([20, 26, 38]));
        for (i, (label, steps)) in samples.into_iter().enumerate() {
            let key = Key { label: label.into(), steps, ..Default::default() };
            let jpeg = crate::key_render::jpeg(&key, size, &font).unwrap();
            // Le JPEG est dans l'orientation du périphérique (retourné) : remis à l'endroit pour la planche.
            let tile = image::imageops::rotate180(&image::load_from_memory(&jpeg).unwrap().to_rgb8());
            image::imageops::overlay(&mut sheet, &tile, (4 + (i % 8) as u32 * (size + 8)) as i64, (4 + (i / 8) as u32 * (size + 8)) as i64);
        }
        sheet.save(std::path::Path::new(&dir).join("touches-icones.png")).unwrap();
    }
}
