use super::*;

#[derive(PartialEq)]
struct Signature {
    label: String,
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
            self.entries.insert(index, Cached { signature, texture });
        }
        self.entries[&index].texture.clone()
    }
}
