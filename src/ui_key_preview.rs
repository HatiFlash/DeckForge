use super::*;

#[derive(PartialEq)]
struct Signature {
    label: String,
    title_size: u8,
    icon_scale: u8,
    color: [u8; 3],
    background: bool,
    image: String,
    kind: usize,
    has_actions: bool,
    size: u32,
    file: Option<(u64, Option<std::time::SystemTime>)>,
}
impl Signature {
    fn new(key: &Key, size: u32) -> Self {
        Self {
            label: key.label.clone(),
            title_size: key.title_size,
            icon_scale: key.icon_scale,
            color: key.color,
            background: key.background,
            image: key.image.clone(),
            kind: crate::icons::kind(key),
            has_actions: !key.steps.is_empty(),
            size,
            file: if key.image.is_empty() {
                None
            } else {
                std::fs::metadata(&key.image)
                    .ok()
                    .map(|m| (m.len(), m.modified().ok()))
            },
        }
    }
}
struct Cached {
    signature: Signature,
    texture: Result<egui::TextureHandle, String>,
    animation: Vec<(egui::TextureHandle, std::time::Duration)>,
    started: std::time::Instant,
}
#[derive(Default)]
pub(super) struct Cache {
    entries: HashMap<usize, Cached>,
}
impl Cache {
    pub(super) fn texture(
        &mut self,
        ctx: &egui::Context,
        index: usize,
        key: &Key,
        size: u32,
    ) -> Result<egui::TextureHandle, String> {
        let signature = Signature::new(key, size);
        if self
            .entries
            .get(&index)
            .is_none_or(|c| c.signature != signature)
        {
            let texture = crate::key_render::preview(key, size)
                .map(|image| {
                    ctx.load_texture(
                        format!("lcd-preview-{index}"),
                        egui::ColorImage::from_rgb([size as usize, size as usize], &image),
                        egui::TextureOptions::LINEAR,
                    )
                })
                .map_err(|e| e.to_string());
            let animation = if key.image.to_ascii_lowercase().ends_with(".gif") {
                crate::key_render::font().and_then(|font| crate::key_render::animated(key, size, &font))
                    .unwrap_or_default().into_iter().enumerate().filter_map(|(frame_index, frame)| {
                        let upright = image::load_from_memory(&frame.jpeg).ok()?.into_rgb8();
                        let upright = image::imageops::rotate180(&upright);
                        Some((ctx.load_texture(format!("lcd-animation-{index}-{frame_index}"),
                            egui::ColorImage::from_rgb([size as usize, size as usize], &upright), egui::TextureOptions::LINEAR), frame.delay))
                    }).collect()
            } else { Vec::new() };
            self.entries.insert(index, Cached { signature, texture, animation, started: std::time::Instant::now() });
        }
        let cached = &self.entries[&index];
        if !cached.animation.is_empty() {
            let cycle: u128 = cached.animation.iter().map(|(_, delay)| delay.as_millis()).sum();
            let mut ms = cached.started.elapsed().as_millis() % cycle.max(1);
            for (texture, delay) in &cached.animation {
                if ms < delay.as_millis() { ctx.request_repaint_after(*delay - std::time::Duration::from_millis(ms as u64)); return Ok(texture.clone()); }
                ms -= delay.as_millis();
            }
        }
        cached.texture.clone()
    }
}
