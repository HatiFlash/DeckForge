//! Canonical LCD rendering. The editor previews the actual JPEG, in upright orientation.
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use anyhow::{Context, Result};
use image::{AnimationDecoder, Rgb, RgbImage};
use std::io::Cursor;
use std::sync::OnceLock;

pub fn font() -> Result<FontArc> {
    static FONT: OnceLock<std::result::Result<FontArc, String>> = OnceLock::new();
    FONT.get_or_init(|| {
        let fontpath = std::path::PathBuf::from(
            std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()),
        )
        .join("Fonts/segoeui.ttf");
        let bytes =
            std::fs::read(fontpath).map_err(|e| format!("Police Windows indisponible : {e}"))?;
        FontArc::try_from_vec(bytes).map_err(|_| "Police Windows illisible".to_string())
    })
    .clone()
    .map_err(anyhow::Error::msg)
}

pub fn preview(key: &crate::model::Key, size: u32) -> Result<RgbImage> {
    let (bytes, _) = for_device(key, size, &font()?)?;
    // USB expects a 180-degree rotation. Undo only that transport transform;
    // the preview retains the exact pixels/colors/text decoded by the device.
    Ok(image::imageops::rotate180(
        &image::load_from_memory(&bytes)?.into_rgb8(),
    ))
}

fn label_lines<F: Font>(
    label: &str,
    font: &impl ScaleFont<F>,
    width: f32,
    rows: usize,
) -> Vec<String> {
    let measure = |text: &str| {
        text.chars()
            .map(|c| font.h_advance(font.glyph_id(c)))
            .sum::<f32>()
    };
    let text = label.chars().take(48).collect::<String>();
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let joined = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if measure(&joined) <= width {
            line = joined;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        for ch in word.chars() {
            let next = format!("{line}{ch}");
            if !line.is_empty() && measure(&next) > width {
                lines.push(std::mem::take(&mut line));
            }
            line.push(ch);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.len() > rows {
        lines.truncate(rows);
        if let Some(last) = lines.last_mut() {
            while !last.is_empty() && measure(&format!("{last}…")) > width {
                last.pop();
            }
            last.push('…');
        }
    }
    lines
}

pub fn for_device(
    k: &crate::model::Key,
    size: u32,
    font: &FontArc,
) -> Result<(Vec<u8>, Option<String>)> {
    match jpeg(k, size, font) {
        Ok(data) => Ok((data, None)),
        Err(e) => {
            let mut fallback = k.clone();
            fallback.image.clear();
            Ok((
                jpeg(&fallback, size, font)?,
                Some(format!(
                    "Icône de « {} » indisponible : {e}. Icône intégrée utilisée.",
                    k.label
                )),
            ))
        }
    }
}
pub fn for_plugin(k: &crate::model::Key, size: u32, font: &FontArc, bytes: Option<&[u8]>, flash: Option<bool>) -> Result<Vec<u8>> {
    let mut key = k.clone();
    let source = bytes.map(|data| crate::assets::decode_image(data, 512)).transpose()?;
    if source.is_some() { key.image = "<plugin-runtime>".into(); }
    jpeg_with_options(&key, size, font, source.as_ref(), flash)
}
pub fn jpeg(k: &crate::model::Key, size: u32, font: &FontArc) -> Result<Vec<u8>> {
    jpeg_with_options(k, size, font, None, None)
}

fn jpeg_with_options(k: &crate::model::Key, size: u32, font: &FontArc, frame: Option<&image::DynamicImage>, flash: Option<bool>) -> Result<Vec<u8>> {
    anyhow::ensure!((32..=512).contains(&size), "Taille LCD invalide");
    let background = if k.background { k.color } else { [0, 0, 0] };
    let mut image = RgbImage::from_pixel(size, size, Rgb(background));
    if !k.image.is_empty() {
        if frame.is_none() { anyhow::ensure!(
            std::fs::metadata(&k.image)?.len() <= if k.image.to_ascii_lowercase().ends_with(".gif") { 20_000_000 } else { 8_000_000 },
            "Image supérieure à 8 Mo"
        ); }
        let source = match frame {
            Some(frame) => frame.clone(),
            None => crate::assets::decode_image(&std::fs::read(&k.image)?, 512)
                .with_context(|| format!("Image invalide pour {}", k.label))?,
        };
        let icon = source.resize_to_fill(size, size, image::imageops::FilterType::Lanczos3)
            .into_rgba8();
        // At 100%, keep the original pixel path byte-for-byte identical.
        let scaled = if k.icon_scale == 100 { icon } else {
            let side = ((size as f32 * k.icon_scale.clamp(20, 200) as f32 / 100.0).round() as u32).max(1);
            let resized = image::imageops::resize(&icon, side, side, image::imageops::FilterType::Lanczos3);
            let mut canvas = image::RgbaImage::new(size, size);
            image::imageops::overlay(&mut canvas, &resized, (size as i64 - side as i64) / 2, (size as i64 - side as i64) / 2);
            canvas
        };
        for (target, source) in image.pixels_mut().zip(scaled.pixels()) {
            let alpha = source[3] as u32;
            for ch in 0..3 {
                target[ch] =
                    ((source[ch] as u32 * alpha + background[ch] as u32 * (255 - alpha) + 127)
                        / 255) as u8;
            }
        }
    }
    let has_icon = k.image.is_empty() && !k.steps.is_empty();
    if has_icon {
        if k.icon_scale == 100 {
            crate::icons::draw(&mut image, crate::icons::kind(k));
        } else {
            let side = ((size as f32 * k.icon_scale.clamp(20, 200) as f32 / 100.0).round() as u32).max(32);
            let mut overlay = RgbImage::from_pixel(side, side, Rgb(background));
            crate::icons::draw(&mut overlay, crate::icons::kind(k));
            let x0 = (size as i64 - side as i64) / 2;
            let y0 = ((size as f32 * 0.29 - side as f32 * 0.29).round()) as i64;
            for y in 0..size { for x in 0..size {
                let sx = x as i64 - x0;
                let sy = y as i64 - y0;
                if sx >= 0 && sy >= 0 && sx < side as i64 && sy < side as i64 {
                    let src = overlay.get_pixel(sx as u32, sy as u32);
                    if src.0 != background { image.put_pixel(x, y, *src); }
                }
            }}
        }
    }
    if !k.label.is_empty() {
        let scaled = font.as_scaled(PxScale::from(size as f32 * k.title_size as f32 / 100.0));
        let lines = label_lines(
            &k.label,
            &scaled,
            size as f32 - 12.0,
            if has_icon { 2 } else { 3 },
        );
        let height = scaled.height();
        let y0 = if has_icon {
            size as f32 * 0.59
        } else if k.image.is_empty() {
            (size as f32 - height * lines.len() as f32) / 2.0
        } else {
            size as f32 - height * lines.len() as f32 - 4.0
        };
        if !k.image.is_empty() {
            for y in (y0.max(0.0) as u32)..size {
                for x in 0..size {
                    let pixel = image.get_pixel_mut(x, y);
                    for ch in &mut pixel.0 {
                        *ch = (*ch as f32 * 0.25) as u8;
                    }
                }
            }
        }
        for (line_idx, line) in lines.iter().enumerate() {
            let width: f32 = line
                .chars()
                .map(|c| scaled.h_advance(scaled.glyph_id(c)))
                .sum();
            let mut x = (size as f32 - width) / 2.0;
            let y = y0 + line_idx as f32 * height + scaled.ascent();
            for c in line.chars() {
                let id = scaled.glyph_id(c);
                let glyph = id.with_scale_and_position(scaled.scale(), ab_glyph::point(x, y));
                if let Some(outline) = font.outline_glyph(glyph) {
                    let bounds = outline.px_bounds();
                    outline.draw(|gx, gy, coverage| {
                        let px = bounds.min.x as i32 + gx as i32;
                        let py = bounds.min.y as i32 + gy as i32;
                        if px >= 0 && py >= 0 && px < size as i32 && py < size as i32 {
                            let p = image.get_pixel_mut(px as u32, py as u32);
                            for channel in &mut p.0 {
                                *channel =
                                    (*channel as f32 * (1.0 - coverage) + 255.0 * coverage) as u8;
                            }
                        }
                    });
                }
                x += scaled.h_advance(id);
            }
        }
    }
    if let Some(badge) = &k.runtime_badge {
        // Device pixels are composed here, before the transport rotation; preview uses this JPEG.
        let red = !matches!(badge.as_str(), "SCÈNE" | "×");
        let color = if red { Rgb([220, 40, 53]) } else { Rgb([40, 146, 192]) };
        let radius = (size / 13).max(4) as i32;
        let cx = size as i32 - radius - 6;
        let cy = radius + 6;
        for y in (cy-radius).max(0)..(cy+radius).min(size as i32) {
            for x in (cx-radius).max(0)..(cx+radius).min(size as i32) {
                if (x-cx).pow(2)+(y-cy).pow(2) <= radius.pow(2) { image.put_pixel(x as u32,y as u32,color); }
            }
        }
        let text = if badge == "SCÈNE" { "SCENE" } else { badge.as_str() };
        let scaled = font.as_scaled(PxScale::from((size as f32 * 0.12).max(9.0)));
        let mut x = 6.0;
        let y = 5.0 + scaled.ascent();
        for c in text.chars() {
            let id = scaled.glyph_id(c);
            let glyph = id.with_scale_and_position(scaled.scale(), ab_glyph::point(x, y));
            if let Some(outline) = font.outline_glyph(glyph) {
                let bounds = outline.px_bounds();
                outline.draw(|gx, gy, coverage| {
                    let px = bounds.min.x as i32 + gx as i32;
                    let py = bounds.min.y as i32 + gy as i32;
                    if px >= 0 && py >= 0 && px < size as i32 && py < size as i32 {
                        let p = image.get_pixel_mut(px as u32, py as u32);
                        for ch in &mut p.0 { *ch = (*ch as f32 * (1.0 - coverage) + 255.0 * coverage) as u8; }
                    }
                });
            }
            x += scaled.h_advance(id);
        }
    }
    if let Some(ok) = flash {
        let center = size as i32 / 2;
        let radius = size as i32 / 6;
        for y in -radius..=radius { for x in -radius..=radius {
            if x*x+y*y <= radius*radius {
                image.put_pixel((center+x) as u32, (center+y) as u32,
                    Rgb(if ok {[25,165,73]}else{[240,142,25]}));
            }
        }}
        let mark = if ok { '✓' } else { '!' };
        let scaled = font.as_scaled(PxScale::from(size as f32 * 0.28));
        let id = scaled.glyph_id(mark);
        let glyph = id.with_scale_and_position(scaled.scale(), ab_glyph::point(center as f32 - scaled.h_advance(id)/2.0, center as f32 + scaled.ascent()/2.0));
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|gx,gy,coverage| {
                let x=bounds.min.x as i32+gx as i32;let y=bounds.min.y as i32+gy as i32;
                if x>=0&&y>=0&&x<size as i32&&y<size as i32 {
                    let p=image.get_pixel_mut(x as u32,y as u32);
                    for c in &mut p.0 {*c=(*c as f32*(1.0-coverage)+255.0*coverage) as u8;}
                }
            });
        }
    }
    let image = image::imageops::rotate180(&image);
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 88).encode(
        &image,
        size,
        size,
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(bytes)
}

/// The same canonical JPEG path is used for the still preview, D6 upload and animation.
pub struct AnimatedFrame {
    pub jpeg: Vec<u8>,
    pub delay: std::time::Duration,
}
pub fn animated(key: &crate::model::Key, size: u32, font: &FontArc) -> Result<Vec<AnimatedFrame>> {
    if !key.image.to_ascii_lowercase().ends_with(".gif") { return Ok(vec![]); }
    let bytes = std::fs::read(&key.image)?;
    anyhow::ensure!(bytes.len() <= 20_000_000, "GIF supérieur à 20 Mo");
    let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes))?;
    let (width, height) = image::ImageDecoder::dimensions(&decoder);
    anyhow::ensure!(width <= 2048 && height <= 2048, "GIF trop grand : réimporter pour réduire à 256 px");
    let mut frames = Vec::new();
    let mut elapsed = 0u32;
    let mut sampled = 0u32;
    for frame in decoder.into_frames().take(151) {
        let frame = frame?;
        anyhow::ensure!(frames.len() < 150 || elapsed >= 10_000, "GIF supérieur à 150 images");
        let (num, den) = frame.delay().numer_denom_ms();
        let duration = num.checked_div(den).unwrap_or(100).max(20);
        if elapsed >= 10_000 { break; }
        elapsed = elapsed.saturating_add(duration);
        if frames.is_empty() || elapsed.saturating_sub(sampled) >= 50 {
            sampled = elapsed;
            frames.push(AnimatedFrame {
                jpeg: jpeg_with_options(key, size, font, Some(&image::DynamicImage::ImageRgba8(frame.into_buffer())), None)?,
                delay: std::time::Duration::from_millis(duration.max(50) as u64),
            });
        }
    }
    if frames.len() < 2 { frames.clear(); }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Key, Step};
    #[test]
    fn animated_gif_uses_the_canonical_jpeg_and_bounded_frames() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("two.gif");
        let mut bytes = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
            encoder.set_repeat(image::codecs::gif::Repeat::Infinite).unwrap();
            for color in [image::Rgba([255, 0, 0, 255]), image::Rgba([0, 255, 0, 255])] {
                encoder.encode_frame(image::Frame::from_parts(
                    image::RgbaImage::from_pixel(16, 16, color), 0, 0,
                    image::Delay::from_numer_denom_ms(100, 1))).unwrap();
            }
        }
        std::fs::write(&path, &bytes).unwrap();
        let key = Key { image: path.to_string_lossy().into_owned(), icon_scale: 60, label: "GIF".into(), ..Key::default() };
        let frames = animated(&key, 126, &font().unwrap()).unwrap();
        assert_eq!(frames.len(), 2);
        assert_ne!(frames[0].jpeg, frames[1].jpeg);
        assert_eq!(frames[0].jpeg, jpeg(&key, 126, &font().unwrap()).unwrap());
        let imported = crate::assets::import_image(&bytes, dir.path(), "two").unwrap();
        assert_eq!(imported.extension().unwrap(), "gif");
        assert_eq!(std::fs::read(imported).unwrap(), bytes);
    }

    #[test]
    fn icon_scale_preserves_legacy_pixels_and_changes_image_and_symbol() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("icon.png");
        let mut img = image::RgbaImage::from_pixel(32, 32, image::Rgba([255, 0, 0, 255]));
        img.put_pixel(0, 0, image::Rgba([0, 255, 0, 255]));
        img.save(&path).unwrap();
        let mut key = Key { image: path.to_string_lossy().into_owned(), ..Key::default() };
        let legacy: Key = serde_json::from_value(serde_json::to_value(&key).unwrap()).unwrap();
        assert_eq!(legacy.icon_scale, 100);
        let original = jpeg(&key, 126, &font().unwrap()).unwrap();
        assert_eq!(original, jpeg(&legacy, 126, &font().unwrap()).unwrap());
        key.icon_scale = 50;
        assert_ne!(original, jpeg(&key, 126, &font().unwrap()).unwrap());
        key.image.clear(); key.steps = vec![Step::Delay { ms: 1 }];
        let small = jpeg(&key, 126, &font().unwrap()).unwrap();
        key.icon_scale = 100;
        assert_ne!(small, jpeg(&key, 126, &font().unwrap()).unwrap());
        key.icon_scale = 19;
        assert!(crate::model::Config::default().validate().is_ok());
    }

    #[test]
    fn plugin_image_flash_and_canonical_orientation() {
        let key=Key{label:"Plugin".into(),..Key::default()};
        let mut png=Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(16,16,image::Rgba([240,20,10,255])))
            .write_to(&mut std::io::Cursor::new(&mut png),image::ImageFormat::Png).unwrap();
        let font=font().unwrap();
        let normal=for_plugin(&key,126,&font,Some(&png),None).unwrap();
        assert_ne!(normal,for_device(&key,126,&font).unwrap().0);
        let flash=for_plugin(&key,126,&font,Some(&png),Some(true)).unwrap();
        assert_ne!(normal,flash);
        assert_ne!(flash,for_plugin(&key,126,&font,Some(&png),Some(false)).unwrap());
        let upright=image::imageops::rotate180(&image::load_from_memory(&normal).unwrap().into_rgb8());
        assert_eq!(upright.dimensions(),(126,126));
    }

    #[test]
    fn title_size_changes_canonical_jpeg_and_legacy_keeps_normal() {
        let mut key = Key { label: "Titre D6".into(), ..Key::default() };
        let legacy = serde_json::to_value(&key).unwrap();
        assert!(legacy.get("title_size").is_none());
        let loaded: Key = serde_json::from_value(legacy).unwrap();
        assert_eq!(loaded.title_size, 14);
        let normal = jpeg(&key, 126, &font().unwrap()).unwrap();
        assert_eq!(normal, jpeg(&loaded, 126, &font().unwrap()).unwrap());
        key.title_size = 20;
        assert_ne!(normal, jpeg(&key, 126, &font().unwrap()).unwrap());
        assert_eq!(preview(&key, 126).unwrap(), image::imageops::rotate180(&image::load_from_memory(&jpeg(&key, 126, &font().unwrap()).unwrap()).unwrap().into_rgb8()));
    }

    #[test]
    fn black_background_and_transparent_images_use_the_saved_background_choice() {
        let dir = tempfile::tempdir().unwrap();
        let mut key = Key {
            color: [160, 80, 40],
            background: false,
            ..Key::default()
        };
        let black = preview(&key, 126).unwrap();
        assert!(black.pixels().all(|p| p.0 == [0, 0, 0]));
        key.background = true;
        let colored = preview(&key, 126).unwrap();
        for (actual, expected) in colored.get_pixel(0, 0).0.into_iter().zip(key.color) {
            assert!(actual.abs_diff(expected) <= 3);
        }
        let image = dir.path().join("transparent.png");
        image::RgbaImage::from_pixel(32, 32, image::Rgba([240, 0, 0, 0]))
            .save(&image)
            .unwrap();
        key.image = image.to_string_lossy().into_owned();
        let colored = preview(&key, 126).unwrap();
        for (actual, expected) in colored.get_pixel(0, 0).0.into_iter().zip(key.color) {
            assert!(actual.abs_diff(expected) <= 3);
        }
        key.background = false;
        assert!(preview(&key, 126)
            .unwrap()
            .pixels()
            .all(|p| p.0 == [0, 0, 0]));
        image::RgbaImage::from_pixel(32, 32, image::Rgba([240, 50, 10, 128]))
            .save(&image)
            .unwrap();
        let blended = preview(&key, 126).unwrap();
        for (actual, expected) in blended.get_pixel(0, 0).0.into_iter().zip([120u8, 25, 5]) {
            assert!(actual.abs_diff(expected) <= 3);
        }
    }

    #[test]
    fn preview_is_the_exact_upright_device_jpeg_for_every_builtin_icon() {
        let font = font().unwrap();
        for size in [112, 126] {
            for kind in 0..=30 {
                let key = Key {
                    label: "Page précédente".into(),
                    color: [63, 103, 181],
                    steps: vec![Step::Delay { ms: 1 }],
                    display_kind: Some(kind),
                    ..Key::default()
                };
                let (bytes, warning) = for_device(&key, size, &font).unwrap();
                assert!(warning.is_none());
                let actual = image::load_from_memory(&bytes).unwrap().into_rgb8();
                assert_eq!(
                    preview(&key, size).unwrap(),
                    image::imageops::rotate180(&actual)
                );
            }
        }
    }

    #[test]
    fn labels_wrap_at_words_and_truncate_long_titles_with_an_ellipsis() {
        let font = font().unwrap();
        let scaled = font.as_scaled(PxScale::from(126.0 * 0.14));
        let lines = label_lines("Page précédente", &scaled, 114.0, 2);
        assert_eq!(lines.join(" "), "Page précédente");
        assert!(lines.len() <= 2);
        let lines = label_lines("Exemple macro", &scaled, 114.0, 2);
        assert_eq!(lines.join(" "), "Exemple macro");
        let lines = label_lines(
            "Une automatisation avec beaucoup de mots à lire",
            &scaled,
            114.0,
            2,
        );
        assert_eq!(lines.len(), 2);
        assert!(lines.last().unwrap().ends_with('…'));
        for line in lines {
            let width: f32 = line
                .chars()
                .map(|c| scaled.h_advance(scaled.glyph_id(c)))
                .sum();
            assert!(width <= 114.0);
        }
    }
}

#[cfg(test)]
mod merged_face_tests {
    use super::*;
    use crate::model::{Key, Step};
    #[test]
    fn plugin_pixels_override_builtin_face_but_preserve_native_badge() {
        let font = font().unwrap();
        let key = Key {
            label: "Studio".into(),
            steps: vec![Step::Delay { ms: 1 }],
            runtime_badge: Some("REC".into()),
            ..Key::default()
        };
        let plain = for_device(&key, 126, &font).unwrap().0;
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(RgbImage::from_pixel(32, 32, Rgb([30, 210, 60])))
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let face = for_plugin(&key, 126, &font, Some(&png), None).unwrap();
        assert_ne!(plain, face);
        let without_badge = for_plugin(&Key { runtime_badge: None, ..key }, 126, &font, Some(&png), None).unwrap();
        assert_ne!(face, without_badge);
        let image = image::load_from_memory(&face).unwrap().to_rgb8();
        let center = image.get_pixel(63, 63).0;
        assert!(center[1] > center[0] && center[1] > center[2]);
    }
}