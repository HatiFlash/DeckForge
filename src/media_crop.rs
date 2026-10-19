//! Unified non-destructive media cropping and scaling for Stream Deck keys.
use image::{imageops, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CropMode {
    Fill,
    Fit,
}

impl Default for CropMode {
    fn default() -> Self {
        CropMode::Fill
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CropSettings {
    pub mode: CropMode,
    pub zoom: f32,
    pub pan_x: f32, // -1.0 ..= 1.0
    pub pan_y: f32, // -1.0 ..= 1.0
    pub bg_color: [u8; 4],
}

impl Default for CropSettings {
    fn default() -> Self {
        Self {
            mode: CropMode::Fill,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            bg_color: [21, 28, 41, 255], // matches DeckForge dark theme
        }
    }
}

/// Crops and scales an `RgbaImage` according to `CropSettings`, outputting a square image of `target_size` x `target_size`.
pub fn apply_crop(img: &RgbaImage, settings: &CropSettings, target_size: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 || target_size == 0 {
        return RgbaImage::new(target_size.max(1), target_size.max(1));
    }
    let zoom = settings.zoom.clamp(0.2, 5.0);
    let target = target_size as f32;

    match settings.mode {
        CropMode::Fill => {
            let base_scale = target / (w.min(h) as f32);
            let scale = base_scale * zoom;
            let nw = ((w as f32 * scale).round() as u32).max(1);
            let nh = ((h as f32 * scale).round() as u32).max(1);
            let scaled = imageops::resize(img, nw, nh, imageops::FilterType::Triangle);

            let cx = (nw as f32 / 2.0) + settings.pan_x.clamp(-1.5, 1.5) * (nw as f32 / 2.0);
            let cy = (nh as f32 / 2.0) + settings.pan_y.clamp(-1.5, 1.5) * (nh as f32 / 2.0);

            let top_left_x = (cx - target / 2.0).round() as i32;
            let top_left_y = (cy - target / 2.0).round() as i32;

            let mut out = RgbaImage::from_pixel(target_size, target_size, Rgba(settings.bg_color));
            for y in 0..target_size {
                let sy = top_left_y + y as i32;
                if sy < 0 || sy >= nh as i32 {
                    continue;
                }
                for x in 0..target_size {
                    let sx = top_left_x + x as i32;
                    if sx >= 0 && sx < nw as i32 {
                        out.put_pixel(x, y, *scaled.get_pixel(sx as u32, sy as u32));
                    }
                }
            }
            out
        }
        CropMode::Fit => {
            let base_scale = target / (w.max(h) as f32);
            let scale = base_scale * zoom;
            let nw = ((w as f32 * scale).round() as u32).max(1);
            let nh = ((h as f32 * scale).round() as u32).max(1);
            let scaled = imageops::resize(img, nw, nh, imageops::FilterType::Triangle);

            let cx = (target / 2.0) + settings.pan_x.clamp(-1.5, 1.5) * (target / 2.0);
            let cy = (target / 2.0) + settings.pan_y.clamp(-1.5, 1.5) * (target / 2.0);

            let dest_x = (cx - nw as f32 / 2.0).round() as i32;
            let dest_y = (cy - nh as f32 / 2.0).round() as i32;

            let mut out = RgbaImage::from_pixel(target_size, target_size, Rgba(settings.bg_color));
            for y in 0..nh {
                let ty = dest_y + y as i32;
                if ty < 0 || ty >= target_size as i32 {
                    continue;
                }
                for x in 0..nw {
                    let tx = dest_x + x as i32;
                    if tx >= 0 && tx < target_size as i32 {
                        let p = *scaled.get_pixel(x, y);
                        if p[3] == 255 {
                            out.put_pixel(tx as u32, ty as u32, p);
                        } else if p[3] > 0 {
                            let bg = out.get_pixel_mut(tx as u32, ty as u32);
                            let alpha = p[3] as f32 / 255.0;
                            let inv = 1.0 - alpha;
                            bg.0[0] = (p[0] as f32 * alpha + bg.0[0] as f32 * inv) as u8;
                            bg.0[1] = (p[1] as f32 * alpha + bg.0[1] as f32 * inv) as u8;
                            bg.0[2] = (p[2] as f32 * alpha + bg.0[2] as f32 * inv) as u8;
                            bg.0[3] = 255;
                        }
                    }
                }
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_mode_crops_center_by_default() {
        let mut img = RgbaImage::new(100, 50);
        for y in 0..50 {
            for x in 0..100 {
                let color = if x < 50 { [255, 0, 0, 255] } else { [0, 0, 255, 255] };
                img.put_pixel(x, y, Rgba(color));
            }
        }
        let settings = CropSettings::default();
        let cropped = apply_crop(&img, &settings, 50);
        assert_eq!(cropped.dimensions(), (50, 50));
        assert_eq!(cropped.get_pixel(5, 25).0, [255, 0, 0, 255]);
        assert_eq!(cropped.get_pixel(45, 25).0, [0, 0, 255, 255]);
    }

    #[test]
    fn fit_mode_adds_letterbox_bands() {
        let img = RgbaImage::from_pixel(100, 50, Rgba([255, 255, 0, 255]));
        let settings = CropSettings {
            mode: CropMode::Fit,
            bg_color: [0, 0, 0, 255],
            ..Default::default()
        };
        let cropped = apply_crop(&img, &settings, 100);
        assert_eq!(cropped.dimensions(), (100, 100));
        assert_eq!(cropped.get_pixel(50, 0).0, [0, 0, 0, 255]);
        assert_eq!(cropped.get_pixel(50, 99).0, [0, 0, 0, 255]);
        assert_eq!(cropped.get_pixel(50, 50).0, [255, 255, 0, 255]);
    }

    #[test]
    fn pan_shifts_visible_area() {
        let mut img = RgbaImage::new(100, 50);
        for y in 0..50 {
            for x in 0..100 {
                let color = if x < 30 { [255, 0, 0, 255] } else { [0, 255, 0, 255] };
                img.put_pixel(x, y, Rgba(color));
            }
        }
        let left_settings = CropSettings {
            pan_x: -0.8,
            ..Default::default()
        };
        let cropped = apply_crop(&img, &left_settings, 50);
        assert_eq!(cropped.get_pixel(25, 25).0, [255, 0, 0, 255]);
    }
}
