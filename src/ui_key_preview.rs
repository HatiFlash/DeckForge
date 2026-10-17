use super::*;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

#[derive(Clone, PartialEq)]
struct Signature {
    face_version: u64,
    label: String,
    runtime_badge: Option<String>,
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
    fn new(key: &Key, size: u32, face: Option<&PluginFace>) -> Self {
        Self {
            face_version: face.map_or(0,|f|f.version),
            label: key.label.clone(), runtime_badge: key.runtime_badge.clone(), title_size: key.title_size, icon_scale: key.icon_scale,
            color: key.color, background: key.background, image: key.image.clone(),
            kind: crate::icons::kind(key), has_actions: !key.steps.is_empty(), size,
            file: if key.image.is_empty() { None } else {
                std::fs::metadata(&key.image).ok().map(|m| (m.len(), m.modified().ok()))
            },
        }
    }
}
type Frames = Vec<(egui::ColorImage, Duration)>;
struct Cached {
    signature: Signature,
    texture: Result<egui::TextureHandle, String>,
    animation: Frames,
    started: Instant,
    selected: Option<usize>,
    changed: Instant,
    loading: bool,
}
pub(super) struct Cache {
    entries: HashMap<usize, Cached>,
    tx: Sender<(usize, Signature, Frames)>,
    rx: Receiver<(usize, Signature, Frames)>,
    next_repaint: Option<Duration>,
    diagnostic: (u32,u32,u32,u32),
}
impl Default for Cache {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { entries: HashMap::new(), tx, rx, next_repaint: None, diagnostic: (0,0,0,0) }
    }
}
fn selected_frame(frames: &Frames, elapsed: Duration) -> (usize, Duration) {
    let cycle: u128 = frames.iter().map(|(_, delay)| delay.as_millis().max(80)).sum();
    let mut ms = elapsed.as_millis() % cycle.max(1);
    for (i, (_, delay)) in frames.iter().enumerate() {
        let span = delay.as_millis().max(80);
        if ms < span { return (i, Duration::from_millis((span - ms) as u64)); }
        ms -= span;
    }
    (0, Duration::from_millis(80))
}
impl Cache {
    pub(super) fn begin_frame(&mut self) {
        self.next_repaint = None;
        self.diagnostic = (0,0,0,0);
        while let Ok((index, signature, frames)) = self.rx.try_recv() {
            if let Some(cached) = self.entries.get_mut(&index) {
                if cached.signature == signature {
                    cached.animation = frames;
                    cached.started = Instant::now();
                    cached.loading = false;
                }
            }
        }
    }
    pub(super) fn diagnostic_stats(&self) -> (u32,u32,u32,u32,Option<u64>) {let (a,b,c,d)=self.diagnostic;(a,b,c,d,self.next_repaint.map(|x|x.max(Duration::from_millis(40)).as_millis() as u64))}
    pub(super) fn end_frame(&self, ctx: &egui::Context) {
        if let Some(delay) = self.next_repaint { ctx.request_repaint_after(delay.max(Duration::from_millis(40))); }
    }
    pub(super) fn texture(
        &mut self, ctx: &egui::Context, index: usize, key: &Key, size: u32, face: Option<&PluginFace>,
    ) -> Result<egui::TextureHandle, String> {
        let signature = Signature::new(key, size, face);
        if self.entries.get(&index).is_none_or(|c| c.signature != signature) {
            self.diagnostic.1 += 1;
            self.diagnostic.2 += 1; // first load or signature change (no asset path recorded)
            let texture = (|| -> anyhow::Result<image::RgbImage> {
                if let Some(face)=face {
                    let jpeg=crate::key_render::for_plugin(key,size,&crate::key_render::font()?,face.image_bytes.as_deref(),face.flash.filter(|(_,when)|when.elapsed()<Duration::from_secs(1)).map(|(ok,_)|ok))?;
                    return Ok(image::imageops::rotate180(&image::load_from_memory(&jpeg)?.into_rgb8()));
                }
                crate::key_render::preview(key,size)
            })()
                .map(|image| ctx.load_texture(format!("lcd-preview-{index}"),
                    egui::ColorImage::from_rgb([size as usize, size as usize], &image),
                    egui::TextureOptions::LINEAR))
                .map_err(|e| e.to_string());
            let loading = face.is_none() && key.image.to_ascii_lowercase().ends_with(".gif") && texture.is_ok();
            self.entries.insert(index, Cached {
                signature: signature.clone(), texture, animation: Vec::new(),
                started: Instant::now(), selected: None,
                changed: Instant::now() - Duration::from_millis(80), loading,
            });
            if loading {
                let tx = self.tx.clone();
                let key = key.clone();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let frames = crate::key_render::font()
                        .and_then(|font| crate::key_render::animated(&key, size, &font))
                        .unwrap_or_default().into_iter().filter_map(|frame| {
                            let upright = image::load_from_memory(&frame.jpeg).ok()?.into_rgb8();
                            let upright = image::imageops::rotate180(&upright);
                            Some((egui::ColorImage::from_rgb([size as usize, size as usize], &upright), frame.delay))
                        }).collect();
                    let _ = tx.send((index, signature, frames));
                    ctx.request_repaint();
                });
            }
        }
        else {self.diagnostic.0 += 1;}
        let cached = self.entries.get_mut(&index).expect("inserted above");
        if !cached.animation.is_empty() {
            let (selected, remaining) = selected_frame(&cached.animation, cached.started.elapsed());
            let until_change = Duration::from_millis(80).saturating_sub(cached.changed.elapsed());
            if cached.selected != Some(selected) && until_change.is_zero() {
                if let Ok(texture) = &mut cached.texture {
                    texture.set(cached.animation[selected].0.clone(), egui::TextureOptions::LINEAR);
                    cached.selected = Some(selected);
                    cached.changed = Instant::now();
                    self.diagnostic.3 += 1;
                }
            }
            let delay = if until_change.is_zero() { remaining } else { remaining.max(until_change) };
            self.next_repaint = Some(self.next_repaint.map_or(delay, |old| old.min(delay)));
        }
        cached.texture.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn animation_keeps_one_texture_and_bounds_repaint() {
        let ctx = egui::Context::default();
        let mut cache = Cache::default();
        let key = Key::default();
        let mut first_id = None;
        let first = ctx.run(Default::default(), |ctx| {
            cache.begin_frame();
            let texture = cache.texture(ctx, 0, &key, 126, None).unwrap();
            cache.end_frame(ctx);
            first_id = Some(texture.id());
        });
        assert_eq!(first.textures_delta.set.iter().filter(|(id, _)| Some(*id) == first_id).count(), 1);
        let image = egui::ColorImage::new([126, 126], egui::Color32::RED);
        cache.entries.get_mut(&0).unwrap().animation = vec![
            (image.clone(), Duration::from_millis(20)),
            (image, Duration::from_millis(20)),
        ];
        let mut second_id = None;
        let second = ctx.run(Default::default(), |ctx| {
            cache.begin_frame();
            let texture = cache.texture(ctx, 0, &key, 126, None).unwrap();
            cache.end_frame(ctx);
            second_id = Some(texture.id());
        });
        assert_eq!(first_id, second_id);
        assert_eq!(second.textures_delta.set.iter().filter(|(id, _)| Some(*id) == second_id).count(), 1);
        let third = ctx.run(Default::default(), |ctx| {
            cache.begin_frame();
            let _ = cache.texture(ctx, 0, &key, 126, None).unwrap();
            cache.end_frame(ctx);
        });
        assert!(third.textures_delta.set.iter().all(|(id, _)| Some(*id) != first_id));
        assert!(cache.next_repaint.unwrap() >= Duration::from_millis(40));
        assert_eq!(selected_frame(&cache.entries[&0].animation, Duration::from_millis(79)).0, 0);
        assert_eq!(selected_frame(&cache.entries[&0].animation, Duration::from_millis(80)).0, 1);
    }
}
