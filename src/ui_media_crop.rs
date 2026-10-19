//! Interactive non-destructive media cropper dialog for Stream Deck keys.
use crate::media_crop::{apply_crop, CropMode, CropSettings};
use anyhow::{anyhow, ensure, Result};
use eframe::egui::{self, Color32, ColorImage, Context, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use image::{AnimationDecoder, RgbaImage};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

pub enum CropDialogEvent {
    Completed(PathBuf),
    Failed(String),
}

pub struct CropSession {
    pub source_path: PathBuf,
    pub is_video: bool,
    pub original_frames: Vec<RgbaImage>,
    pub display_frames: Vec<RgbaImage>,
    pub current_frame: usize,
    pub playing: bool,
    pub last_play_tick: Instant,
    pub settings: CropSettings,
    pub profile: usize,
    pub page: String,
    pub index: usize,
    pub size: u32,
    pub assets_dir: PathBuf,
    pub source_texture: Option<TextureHandle>,
    pub preview_texture: Option<TextureHandle>,
    pub last_settings_hash: u64,
    pub processing: bool,
    pub tx: Sender<CropDialogEvent>,
    pub rx: Receiver<CropDialogEvent>,
}

impl CropSession {
    pub fn open(
        path: PathBuf,
        profile: usize,
        page: String,
        index: usize,
        size: u32,
        assets_dir: PathBuf,
    ) -> Result<Self> {
        ensure!(path.is_file(), "Fichier introuvable");
        let is_video = crate::assets::is_video(&path);
        let mut original_frames = Vec::new();

        if is_video {
            original_frames = crate::video_mf::decode_raw(&path, 12, 10)
                .map_err(|e| anyhow!("Impossible de lire la vidéo ({e})"))?;
        } else {
            let bytes = std::fs::read(&path)?;
            ensure!(bytes.len() <= 25_000_000, "Image de plus de 25 Mo");
            let is_gif = bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a");
            if is_gif {
                if let Ok(decoder) = image::codecs::gif::GifDecoder::new(Cursor::new(&bytes)) {
                    if let Ok(frames) = decoder.into_frames().collect_frames() {
                        for f in frames {
                            original_frames.push(f.into_buffer());
                        }
                    }
                }
            }
            if original_frames.is_empty() {
                let img = image::load_from_memory(&bytes)
                    .map_err(|e| anyhow!("Format d'image non reconnu ({e})"))?;
                original_frames.push(img.into_rgba8());
            }
        }

        ensure!(!original_frames.is_empty(), "Aucune image exploitable");

        // Pre-downscale display frames if source is oversized, ensuring smooth egui render
        let mut display_frames = Vec::new();
        for frame in &original_frames {
            let (w, h) = frame.dimensions();
            if w > 640 || h > 640 {
                let scale = 640.0 / (w.max(h) as f32);
                let nw = ((w as f32 * scale).round() as u32).max(1);
                let nh = ((h as f32 * scale).round() as u32).max(1);
                display_frames.push(image::imageops::resize(frame, nw, nh, image::imageops::FilterType::Triangle));
            } else {
                display_frames.push(frame.clone());
            }
        }

        let (tx, rx) = mpsc::channel();

        Ok(Self {
            source_path: path,
            is_video,
            original_frames,
            display_frames,
            current_frame: 0,
            playing: is_video,
            last_play_tick: Instant::now(),
            settings: CropSettings::default(),
            profile,
            page,
            index,
            size: if size > 0 { size } else { 112 },
            assets_dir,
            source_texture: None,
            preview_texture: None,
            last_settings_hash: 0,
            processing: false,
            tx,
            rx,
        })
    }

    fn hash_state(&self) -> u64 {
        let mut h = self.current_frame as u64;
        h = h.wrapping_mul(31).wrapping_add(self.settings.mode as u64);
        h = h.wrapping_mul(31).wrapping_add((self.settings.zoom * 1000.0) as u64);
        h = h.wrapping_mul(31).wrapping_add((self.settings.pan_x * 1000.0) as i64 as u64);
        h = h.wrapping_mul(31).wrapping_add((self.settings.pan_y * 1000.0) as i64 as u64);
        h
    }

    pub fn ui(&mut self, ctx: &Context, open: &mut bool) -> Option<Result<PathBuf>> {
        // Check async export results
        if let Ok(event) = self.rx.try_recv() {
            self.processing = false;
            match event {
                CropDialogEvent::Completed(path) => {
                    *open = false;
                    return Some(Ok(path));
                }
                CropDialogEvent::Failed(err) => {
                    return Some(Err(anyhow!(err)));
                }
            }
        }

        // Video playback ticker
        if self.playing && self.original_frames.len() > 1 {
            if self.last_play_tick.elapsed() >= Duration::from_millis(83) { // ~12 fps
                self.current_frame = (self.current_frame + 1) % self.original_frames.len();
                self.last_play_tick = Instant::now();
            }
            ctx.request_repaint_after(Duration::from_millis(83));
        }

        let current_display = &self.display_frames[self.current_frame.min(self.display_frames.len() - 1)];
        let current_orig = &self.original_frames[self.current_frame.min(self.original_frames.len() - 1)];

        // Update source texture if frame changed
        let frame_id = format!("crop-src-{}-{}
", self.source_path.display(), self.current_frame);
        let color_src = ColorImage::from_rgba_unmultiplied(
            [current_display.width() as usize, current_display.height() as usize],
            current_display.as_raw(),
        );
        if let Some(tex) = &mut self.source_texture {
            tex.set(color_src, TextureOptions::LINEAR);
        } else {
            self.source_texture = Some(ctx.load_texture(frame_id, color_src, TextureOptions::LINEAR));
        }

        // Update key preview texture if settings or frame changed
        let state_hash = self.hash_state();
        if self.last_settings_hash != state_hash || self.preview_texture.is_none() {
            let cropped = apply_crop(current_orig, &self.settings, self.size);
            let color_prev = ColorImage::from_rgba_unmultiplied(
                [self.size as usize, self.size as usize],
                cropped.as_raw(),
            );
            if let Some(tex) = &mut self.preview_texture {
                tex.set(color_prev, TextureOptions::LINEAR);
            } else {
                self.preview_texture = Some(ctx.load_texture(
                    "crop-key-preview",
                    color_prev,
                    TextureOptions::LINEAR,
                ));
            }
            self.last_settings_hash = state_hash;
        }

        let mut apply_action = false;
        let title = if self.is_video {
            "Recadrage vidéo pour touche FIFINE D6"
        } else {
            "Recadrage image pour touche FIFINE D6"
        };

        egui::Window::new(title)
            .open(open)
            .collapsible(false)
            .default_width(740.0)
            .default_height(500.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(
                        self.source_path.file_name().and_then(|n| n.to_str()).unwrap_or("Média"),
                    ).strong());
                    let (orig_w, orig_h) = self.original_frames[0].dimensions();
                    ui.label(format!("({orig_w}x{orig_h} px)"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("Fichier original préservé").small().color(Color32::from_rgb(148, 163, 184)));
                    });
                });

                ui.add_space(4.0);

                // Mode and zoom control bar
                ui.horizontal(|ui| {
                    ui.label("Mode :");
                    ui.selectable_value(&mut self.settings.mode, CropMode::Fill, "Remplir (recadrer)");
                    ui.selectable_value(&mut self.settings.mode, CropMode::Fit, "Ajuster (bandes)");
                    ui.separator();
                    ui.label("Zoom :");
                    ui.add(egui::Slider::new(&mut self.settings.zoom, 0.5..=3.0).step_by(0.05));
                    if ui.button("Recentrer").on_hover_text("Réinitialiser le zoom et le positionnement au centre").clicked() {
                        self.settings.pan_x = 0.0;
                        self.settings.pan_y = 0.0;
                        self.settings.zoom = 1.0;
                    }
                });

                ui.separator();

                // Split workspace: Editor on Left, Target D6 key on Right
                ui.columns(2, |cols| {
                    // Left column: Interactive canvas
                    cols[0].vertical(|ui| {
                        ui.label(RichText::new("Cadrage interactif (glisser pour déplacer)").strong().small());
                        let (w, h) = current_display.dimensions();
                        let aspect = w as f32 / h.max(1) as f32;
                        let canvas_w = ui.available_width().min(380.0);
                        let canvas_h = (canvas_w / aspect).min(280.0);
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(canvas_w, canvas_h), Sense::drag());

                        if let Some(tex) = &self.source_texture {
                            ui.painter().image(tex.id(), rect, Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)), Color32::WHITE);
                        }

                        // Pan via drag
                        if resp.dragged() {
                            let delta = resp.drag_delta();
                            self.settings.pan_x += (delta.x / (canvas_w * 0.5)) / self.settings.zoom.max(0.1);
                            self.settings.pan_y += (delta.y / (canvas_h * 0.5)) / self.settings.zoom.max(0.1);
                            self.settings.pan_x = self.settings.pan_x.clamp(-1.5, 1.5);
                            self.settings.pan_y = self.settings.pan_y.clamp(-1.5, 1.5);
                        }

                        // Zoom via mouse wheel when hovering editor
                        if resp.hovered() {
                            let scroll = ui.input(|i| i.raw_scroll_delta.y);
                            if scroll != 0.0 {
                                self.settings.zoom = (self.settings.zoom + scroll * 0.002).clamp(0.2, 5.0);
                            }
                        }

                        // Visual overlay: square target viewfinder
                        let box_size = canvas_w.min(canvas_h) * 0.8;
                        let box_rect = Rect::from_center_size(rect.center(), Vec2::splat(box_size));
                        ui.painter().rect_stroke(box_rect, 4.0, Stroke::new(2.0, Color32::from_rgb(59, 130, 246)));

                        // If video: playback controls
                        if self.original_frames.len() > 1 {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button(if self.playing { "❚❚ Pause" } else { "▶ Lecture" }).clicked() {
                                    self.playing = !self.playing;
                                }
                                ui.add(egui::Slider::new(&mut self.current_frame, 0..=self.original_frames.len() - 1).show_value(false));
                                ui.label(format!("{}/{}", self.current_frame + 1, self.original_frames.len()));
                            });
                        }
                    });

                    // Right column: Final D6 Key rendering
                    cols[1].vertical(|ui| {
                        ui.label(RichText::new("Aperçu fidèle sur touche D6").strong().small());
                        ui.label(RichText::new(format!("Format final : {}x{} px", self.size, self.size)).small().color(Color32::from_rgb(148, 163, 184)));
                        
                        ui.add_space(10.0);
                        let key_preview_size = 140.0;
                        let (k_rect, _) = ui.allocate_exact_size(Vec2::splat(key_preview_size), Sense::hover());

                        // Draw simulated key bezel
                        ui.painter().rect_filled(k_rect.expand(6.0), 16.0, Color32::from_rgb(15, 23, 42));
                        ui.painter().rect_stroke(k_rect.expand(6.0), 16.0, Stroke::new(1.5, Color32::from_rgb(51, 65, 85)));

                        if let Some(tex) = &self.preview_texture {
                            ui.painter().image(tex.id(), k_rect, Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)), Color32::WHITE);
                        }
                        ui.painter().rect_stroke(k_rect, 10.0, Stroke::new(1.0, Color32::from_rgb(71, 85, 105)));
                    });
                });

                ui.add_space(12.0);
                ui.separator();

                // Action buttons
                ui.horizontal(|ui| {
                    if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        *open = false;
                    }
                    if self.processing {
                        ui.spinner();
                        ui.label("Conversion et optimisation en cours...");
                    } else {
                        let btn = egui::Button::new(RichText::new("Appliquer à la touche").color(Color32::WHITE))
                            .fill(Color32::from_rgb(59, 130, 246));
                        if ui.add(btn).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            apply_action = true;
                        }
                    }
                });
            });

        if apply_action && !self.processing {
            self.processing = true;
            let tx = self.tx.clone();
            let frames = self.original_frames.clone();
            let is_video = self.is_video;
            let settings = self.settings;
            let size = self.size;
            let assets_dir = self.assets_dir.clone();
            let name = self.source_path.file_stem().and_then(|s| s.to_str()).unwrap_or("media").to_string();

            std::thread::spawn(move || {
                let result = (|| -> Result<PathBuf> {
                    std::fs::create_dir_all(&assets_dir)?;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_nanos();
                    if is_video || frames.len() > 1 {
                        let cropped_frames: Vec<_> = frames
                            .iter()
                            .map(|f| apply_crop(f, &settings, size))
                            .collect();
                        let gif_bytes = crate::video_mf::encode_gif(cropped_frames, 12)?;
                        let out_path = assets_dir.join(format!("{name}-crop-{now}.gif"));
                        std::fs::write(&out_path, gif_bytes)?;
                        Ok(out_path)
                    } else {
                        let cropped = apply_crop(&frames[0], &settings, size);
                        let out_path = assets_dir.join(format!("{name}-crop-{now}.png"));
                        cropped.save_with_format(&out_path, image::ImageFormat::Png)?;
                        Ok(out_path)
                    }
                })();
                match result {
                    Ok(path) => { let _ = tx.send(CropDialogEvent::Completed(path)); }
                    Err(e) => { let _ = tx.send(CropDialogEvent::Failed(e.to_string())); }
                }
            });
        }

        None
    }
}
