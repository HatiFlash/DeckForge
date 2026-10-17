//! Canonical LCD rendering. The editor previews the actual JPEG, in upright orientation.
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use anyhow::{Context, Result};
use image::{Rgb, RgbImage};
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
pub fn jpeg(k: &crate::model::Key, size: u32, font: &FontArc) -> Result<Vec<u8>> {
    anyhow::ensure!((32..=512).contains(&size), "Taille LCD invalide");
    let mut image = if k.image.is_empty() {
        RgbImage::from_pixel(size, size, Rgb(k.color))
    } else {
        anyhow::ensure!(
            std::fs::metadata(&k.image)?.len() <= 8_000_000,
            "Image supérieure à 8 Mo"
        );
        crate::assets::decode_image(&std::fs::read(&k.image)?, 512)
            .with_context(|| format!("Image invalide pour {}", k.label))?
            .resize_to_fill(size, size, image::imageops::FilterType::Lanczos3)
            .into_rgb8()
    };
    let has_icon = k.image.is_empty() && !k.steps.is_empty();
    if has_icon {
        crate::icons::draw(&mut image, crate::icons::kind(k));
    }
    if !k.label.is_empty() {
        let scaled = font.as_scaled(PxScale::from(size as f32 * 0.14));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Key, Step};

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
