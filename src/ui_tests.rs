//! Tests of the actual egui widgets, without opening or controlling Windows.

#[test]
fn workshop_ai_requires_consent_and_local_never_offers_it() {
    let mut h = Harness::new();
    h.app.preferences.workshop_chat_mode = "ia".into();
    h.app.workshop_chat_send("une demande".into(), None);
    assert!(h.app.workshop_chat.consent.is_some());
    let scope = (h.app.config.active, h.app.config.page.clone(), h.app.selected);
    assert!(super::workshop_chat_ui::mcp_request(&mut h.app.workshop_chat, &scope).is_none());
    h.app.preferences.workshop_chat_mode = "local".into();
    h.app.workshop_chat_send("inconnue introuvable xyz".into(), None);
    assert!(h.app.workshop_chat.consent.is_none());
    assert!(super::workshop_chat_ui::mcp_request(&mut h.app.workshop_chat, &scope).is_none());
}

#[test]
fn mcp_get_state_exposes_request_only_after_consent_and_not_after_cancel() {
    let mut h = Harness::new();
    let args = serde_json::json!({});
    let state = h.app.assistant_call("deckforge_get_state", &args).unwrap();
    assert!(state.get("pending_request").is_none());
    h.app.preferences.workshop_chat_mode = "ia".into();
    h.app.workshop_chat_send("attends 2 s".into(), None);
    assert!(h.app.assistant_call("deckforge_get_state", &args).unwrap().get("pending_request").is_none());
    h.app.workshop_chat.awaiting = h.app.workshop_chat.consent.take();
    let state = h.app.assistant_call("deckforge_get_state", &args).unwrap();
    assert_eq!(state["pending_request"]["text"], "attends 2 s");
    assert_eq!(state["pending_request"]["destination"]["key"], 1);
    h.app.workshop_chat.awaiting = None;
    assert!(h.app.assistant_call("deckforge_get_state", &args).unwrap().get("pending_request").is_none());
}

#[test]
fn workshop_chat_consent_card_is_visible_without_publishing() {
    let mut h = Harness::new();
    let size = vec2(1366.0, 768.0);
    h.app.show_workflow = true;
    h.app.workshop_chat.open = true;
    h.app.preferences.workshop_chat_mode = "ia".into();
    h.app.workshop_chat_send("aide".into(), None);
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "Transmettre à mon IA").is_some());
    assert!(text_position(&output.shapes, "Non merci").is_some());
    assert!(h.app.workshop_chat.awaiting.is_none());
    assert!(h.app.assistant_call("deckforge_get_state", &serde_json::json!({})).unwrap().get("pending_request").is_none());
    let pos = text_position(&output.shapes, "Non merci").unwrap();
    h.click(size, pos);
    assert!(h.app.workshop_chat.consent.is_none());
}

#[test]
fn workshop_responsive_panels_reserve_space_at_multiple_sizes_and_dpi() {
    for (physical, dpi) in [(vec2(1000.0, 700.0), 1.0), (vec2(1000.0, 700.0), 1.5),
        (vec2(1366.0, 768.0), 1.0), (vec2(1920.0, 1080.0), 1.0),
        (vec2(1366.0, 768.0), 1.25), (vec2(1920.0, 1080.0), 1.5)] {
        let mut h = Harness::new();
        h.ctx.set_pixels_per_point(dpi);
        let size = physical / dpi;
        h.app.show_workflow = true;
        h.app.config.keys_mut()[0].steps = vec![Step::Delay { ms: 100 }];
        h.app.workshop_chat.open = true;
        let output = h.warm(size);
        for label in ["Fermer l’assistant", "Simuler", "Tester pour de vrai", "Enregistrer", "Construis ton automatisation"] {
            assert!(text_position(&output.shapes, label).is_some(), "{label} at {physical:?} / {dpi}");
        }
        if size.x < 1500.0 {
            let chat = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("workshop_chat_panel")).unwrap().rect;
            assert!(chat.left() > 0.0 && chat.right() <= size.x + 1.0, "chat {chat:?} at {size:?}");
            let title = text_position(&output.shapes, "Construis ton automatisation").unwrap();
            assert!(title.x < chat.left() && title.y < size.y, "chat overlaps canvas {chat:?}, title {title:?}");
            assert!(text_position(&output.shapes, "Retour à la séquence").is_some());
            assert!(text_position(&output.shapes, "BIBLIOTHÈQUE DE BLOCS").is_none());
            assert!(text_position(&output.shapes, "Propriétés du bloc").is_none());
        }
        h.app.workshop_chat.open = false;
        h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("workshop_path"), vec![0_usize]));
        let output = h.warm(size);
        assert!(text_position(&output.shapes, "Propriétés du bloc").is_some(), "properties {size:?} panel={:?} path={:?} text={:?}", egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("workshop_properties_panel")).map(|p| p.rect), h.ctx.data(|d| d.get_temp::<Vec<usize>>(egui::Id::new("workshop_path"))), output.shapes.iter().filter_map(|s| if let egui::Shape::Text(t)=&s.shape { Some(t.galley.job.text.clone()) } else { None }).take(30).collect::<Vec<_>>());
        assert!(text_position(&output.shapes, "Construis ton automatisation").is_some());
        let property = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("workshop_properties_panel")).unwrap().rect;
        assert!(property.left() > 0.0 && property.right() <= size.x + 1.0, "property {property:?} at {size:?}");
        assert!(text_position(&output.shapes, "Construis ton automatisation").unwrap().x < property.left());
        assert!(text_position(&output.shapes, "× Fermer les propriétés").is_some());
    }
}

#[test]
fn workshop_advanced_cards_start_folded_and_reveal_real_controls() {
    for (physical, dpi) in [(vec2(1000.0, 700.0), 1.0), (vec2(1366.0, 768.0), 1.25),
        (vec2(1920.0, 1080.0), 1.5)] {
        let size = physical / dpi;
        let mut h = Harness::new();
        h.ctx.set_pixels_per_point(dpi);
        h.app.show_workflow = true;
        h.app.preferences.workshop_advanced = true;
        h.app.config.keys_mut()[0].steps = vec![Step::Delay { ms: 125 }];
        let output = h.warm(size);
        if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
            let snapshot = h.warm(size);
            h.png(size, snapshot, &PathBuf::from(dir).join(format!("atelier-advanced-folded-{}-{}.png", physical.x as u32, (dpi * 100.0) as u32)));
        }
        assert!(text_position(&output.shapes, "ms").is_none(), "folded {physical:?} {dpi}");
        let id = egui::Id::new("block_fold").with(vec![0_usize]);
        h.ctx.data_mut(|d| d.insert_temp(id, false));
        let output = h.warm(size);
        if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
            let snapshot = h.warm(size);
            h.png(size, snapshot, &PathBuf::from(dir).join(format!("atelier-advanced-expanded-{}-{}.png", physical.x as u32, (dpi * 100.0) as u32)));
        }
        assert!(text_position(&output.shapes, "ms").is_some(), "expanded {physical:?} {dpi}");
    }
}

#[test]
fn workshop_chat_and_properties_fit_both_screen_sizes() {
    for size in [vec2(1366.0, 768.0), vec2(1920.0, 1080.0)] {
        let mut h = Harness::new();
        h.app.show_workflow = true;
        h.app.config.keys_mut()[0].steps = vec![Step::Delay { ms: 100 }];
        h.app.workshop_chat.open = true;
        let output = h.warm(size);
        assert!(!output.shapes.is_empty());
        if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
            let snapshot = h.warm(size);
            h.png(size, snapshot, &PathBuf::from(dir).join(format!("atelier-chat-{}.png", size.x as u32)));
        }
        assert!(text_position(&output.shapes, "Assistant · Atelier").is_some());
        let panel = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("workshop_chat_panel")).unwrap().rect;
        let canvas = text_position(&output.shapes, "Construis ton automatisation").unwrap();
        assert!(canvas.x < panel.left() && panel.right() <= size.x + 1.0, "chat {panel:?} canvas {canvas:?}");
    }
}
use super::*;
use egui::{pos2, vec2, Event as Input, Modifiers, PointerButton, Rect};
#[test]
fn workshop_chat_previews_a_natural_request_without_running_it() {
    let mut h = Harness::new();
    let size = vec2(1366.0, 768.0);
    h.app.show_workflow = true;
    h.app.selected = 2;
    let before = h.app.config.keys()[2].steps.clone();
    h.app.workshop_chat.open = true;
    h.app.workshop_chat_send("ferme toutes les fenêtres sauf celle sur laquelle je suis, puis attends 1 s".into(), None);
    let pending = h.app.workshop_chat.pending.as_ref().expect("aperçu");
    assert!(matches!(&pending.proposal.steps[..], [Step::WindowOp { op, .. }, Step::Delay { ms: 1000 }] if op == "close_others"));
    assert_eq!(h.app.config.keys()[2].steps, before, "aucune insertion sans validation");
    h.warm(size);
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "Ajouter").is_some());
    if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
        std::fs::create_dir_all(&dir).unwrap();
        h.png(size, output, &PathBuf::from(dir).join("atelier-assistant.png"));
    }
}
#[test]
fn gallery_imports_a_user_icon_without_overwriting_and_applies_it() {
    let mut h = Harness::new();
    let source_dir = tempfile::tempdir().unwrap();
    let source = source_dir.path().join("mon logo.png");
    image::RgbaImage::from_pixel(40, 40, image::Rgba([10, 200, 90, 255])).save(&source).unwrap();
    let target = icon_ui::Target { profile: h.app.config.active, page: h.app.config.page.clone(), key: 4, destination: icon_ui::Destination::Key };
    let first = h.app.import_icon_from(&target, &source).unwrap();
    assert!(first.is_file());
    assert_eq!(h.app.config.keys()[4].image, first.to_string_lossy());
    let second = h.app.import_icon_from(&target, &source).unwrap();
    assert!(first.is_file() && second.is_file(), "aucun fichier écrasé");
    assert_eq!(h.app.config.keys()[4].image, second.to_string_lossy());
    h.app.show_assets = true;
    h.app.open_icon_gallery(None);
    let output = h.warm(vec2(1380.0, 900.0));
    assert!(text_position(&output.shapes, "Importer une icône…").is_some(), "bouton d'import visible dans la galerie");
}
#[test]
fn workshop_chat_texts_and_buttons_stay_inside_the_panel() {
    // Régression : textes et boutons du chat coupés (messages longs, destination JSON, boutons).
    for (size, dpi) in [(vec2(1000.0, 700.0), 1.0), (vec2(1366.0, 768.0), 1.25), (vec2(1920.0, 1080.0), 1.5)] {
        let mut h = Harness::new();
        h.ctx.set_pixels_per_point(dpi);
        let size = size / dpi;
        h.app.show_workflow = true;
        h.app.config.keys_mut()[2].steps = vec![Step::Delay { ms: 100 }];
        h.app.selected = 2;
        h.app.workshop_chat.open = true;
        h.app.preferences.workshop_chat_mode = "auto".into();
        h.app.workshop_chat_send("programme une alarme pour mon rendez-vous chez le dentiste avec le lien https://exemple.fr/une/adresse/tres/longue/sans/espaces".into(), None);
        assert!(h.app.workshop_chat.consent.is_some());
        h.warm(size);
        let output = h.warm(size);
        let panel = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("workshop_chat_panel")).unwrap().rect;
        if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
            std::fs::create_dir_all(&dir).unwrap();
            h.png(size, output.clone(), &PathBuf::from(dir).join(format!("atelier-chat-consent-{}.png", (size.x * dpi) as u32)));
        }
        let mut seen = 0;
        for shape in &output.shapes {
            if let egui::Shape::Text(t) = &shape.shape {
                let r = t.visual_bounding_rect();
                if t.galley.job.text.trim().is_empty() || !r.is_finite() || r.left() + 0.5 < panel.left() { continue; }
                seen += 1;
                assert!(r.right() <= panel.right() + 1.0, "{:?} dépasse {panel:?} à {size:?}@{dpi}: {r:?}", t.galley.job.text);
            }
        }
        assert!(seen > 8, "chat non rendu ({seen})");
        assert!(h.app.workshop_chat.consent.as_ref().unwrap().destination_label().starts_with("Touche 3 · page"), "destination lisible attendue");
        assert!(!output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text.contains("{\"key\""))), "JSON brut affiché");
        for label in ["Transmettre à mon IA", "Non merci", "Ajouter une temporisation"] {
            let pos = text_position(&output.shapes, label).unwrap_or_else(|| panic!("{label} absent à {size:?}"));
            assert!(pos.y < size.y && pos.x >= panel.left(), "{label} hors du panneau {pos:?}");
        }
    }
}

#[test]
fn grids_render_without_overflow_or_touching_hardware() {
    for (name, cols, rows, keys) in [("Mini", 3, 2, 6), ("D6", 5, 3, 15), ("XL", 8, 4, 32)] {
        let mut h = Harness::new();
        h.app.device_model = DeviceGrid::new(name.into(), keys, rows, cols, name != "D6");
        crate::model::extend_for_device(&mut h.app.config, keys as usize);
        let output = h.warm(vec2(1060.0, 720.0));
        assert_eq!(h.app.drop_targets.len(), keys as usize, "{name}");
        for (rect, index) in &h.app.drop_targets {
            assert!(*index < keys as usize);
            assert!(rect.left() >= 0.0 && rect.top() >= 0.0 && rect.right() <= 1060.0 && rect.bottom() <= 720.0, "{name}: {rect:?}");
        }
        assert!(!output.shapes.is_empty());
        if keys == 6 { assert_eq!(h.app.config.keys().len(), 15); }
        if keys == 32 { assert_eq!(h.app.config.keys().len(), 32); }
    }
}
struct Harness {
    ctx: egui::Context,
    app: App,
    dir: tempfile::TempDir,
    time: f64,
    textures: HashMap<egui::TextureId, egui::ColorImage>,
}
impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = App::init(&ctx, Some(dir.path().join("config.json")));
        // Captures et clics stables : les animations ont leur propre test.
        app.preferences.reduce_motion = true;
        Self {
            ctx,
            app,
            dir,
            time: 0.0,
            textures: HashMap::new(),
        }
    }
    fn frame(&mut self, size: Vec2, events: Vec<Input>) -> egui::FullOutput {
        self.time += 0.05;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(self.time),
            modifiers: events.iter().find_map(|e| if let Input::Key { modifiers, .. } = e { Some(*modifiers) } else { None }).unwrap_or_default(),
            events,
            focused: true,
            ..Default::default()
        };
        let output = self.ctx.run(input, |ctx| { let start=Instant::now(); self.app.frame(ctx); self.app.diagnostic_sample(ctx,start); });
        for (id, delta) in &output.textures_delta.set {
            let image = match &delta.image {
                egui::ImageData::Color(c) => (**c).clone(),
                egui::ImageData::Font(f) => egui::ColorImage {
                    size: f.size,
                    pixels: f.srgba_pixels(None).collect(),
                },
            };
            if let Some([x, y]) = delta.pos {
                let target = self.textures.get_mut(id).unwrap();
                for row in 0..image.size[1] {
                    for col in 0..image.size[0] {
                        target[(x + col, y + row)] = image[(col, row)];
                    }
                }
            } else {
                self.textures.insert(*id, image);
            }
        }
        output
    }
    fn warm(&mut self, size: Vec2) -> egui::FullOutput {
        for _ in 0..3 {
            self.frame(size, vec![]);
        }
        self.frame(size, vec![])
    }
    fn click(&mut self, size: Vec2, pos: egui::Pos2) {
        self.frame(size, vec![Input::PointerMoved(pos), button(pos, true)]);
        self.frame(size, vec![button(pos, false)]);
    }
    fn png(&self, size: Vec2, output: egui::FullOutput, path: &std::path::Path) {
        self.raster(size, output).save(path).unwrap();
    }
    /// Rastérise à la résolution physique (taille logique × pixels_per_point), avec un
    /// échantillonnage bilinéaire comme le filtre LINEAR du rendu GPU : pas d'agrandissement après coup.
    fn raster(&self, size: Vec2, output: egui::FullOutput) -> image::RgbaImage {
        let ppp = output.pixels_per_point;
        let width = (size.x * ppp).round() as u32;
        let height = (size.y * ppp).round() as u32;
        let size = vec2(width as f32, height as f32);
        let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([24, 26, 31, 255]));
        let primitives = self.ctx.tessellate(output.shapes, ppp);
        for primitive in primitives {
            let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
                continue;
            };
            let Some(texture) = self.textures.get(&mesh.texture_id) else {
                continue;
            };
            let clip = Rect::from_min_max(primitive.clip_rect.min * ppp, primitive.clip_rect.max * ppp);
            for triangle in mesh.indices.chunks_exact(3) {
                let scaled = |i: u32| { let mut v = mesh.vertices[i as usize]; v.pos = (v.pos.to_vec2() * ppp).to_pos2(); v };
                let a = scaled(triangle[0]);
                let b = scaled(triangle[1]);
                let c = scaled(triangle[2]);
                let cross = |u: egui::Vec2, v: egui::Vec2| u.x * v.y - u.y * v.x;
                let area = cross(b.pos - a.pos, c.pos - a.pos);
                if area.abs() < 0.00001 {
                    continue;
                }
                let bounds = Rect::from_min_max(
                    pos2(
                        a.pos.x.min(b.pos.x).min(c.pos.x),
                        a.pos.y.min(b.pos.y).min(c.pos.y),
                    ),
                    pos2(
                        a.pos.x.max(b.pos.x).max(c.pos.x),
                        a.pos.y.max(b.pos.y).max(c.pos.y),
                    ),
                )
                .intersect(clip)
                .intersect(Rect::from_min_size(egui::Pos2::ZERO, size));
                for y in (bounds.top().floor().max(0.0) as u32)
                    ..(bounds.bottom().ceil().min(height as f32) as u32)
                {
                    for x in (bounds.left().floor().max(0.0) as u32)
                        ..(bounds.right().ceil().min(width as f32) as u32)
                    {
                        let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                        let wa = cross(b.pos - p, c.pos - p) / area;
                        let wb = cross(c.pos - p, a.pos - p) / area;
                        let wc = 1.0 - wa - wb;
                        if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                            continue;
                        }
                        let uv = a.uv.to_vec2() * wa + b.uv.to_vec2() * wb + c.uv.to_vec2() * wc;
                        let (tw, th) = (texture.size[0], texture.size[1]);
                        let fx = (uv.x * tw as f32 - 0.5).clamp(0.0, (tw - 1) as f32);
                        let fy = (uv.y * th as f32 - 0.5).clamp(0.0, (th - 1) as f32);
                        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
                        let (x1, y1) = ((x0 + 1).min(tw - 1), (y0 + 1).min(th - 1));
                        let (dx, dy) = (fx - x0 as f32, fy - y0 as f32);
                        let mut tex = [0.0_f32; 4];
                        for (px, py, w) in [(x0, y0, (1.0 - dx) * (1.0 - dy)), (x1, y0, dx * (1.0 - dy)), (x0, y1, (1.0 - dx) * dy), (x1, y1, dx * dy)] {
                            let t = texture[(px, py)].to_array();
                            for ch in 0..4 { tex[ch] += t[ch] as f32 * w; }
                        }
                        let ac = a.color.to_array();
                        let bc = b.color.to_array();
                        let cc = c.color.to_array();
                        let mut src = [0.0; 4];
                        for ch in 0..4 {
                            src[ch] =
                                (ac[ch] as f32 * wa + bc[ch] as f32 * wb + cc[ch] as f32 * wc)
                                    * tex[ch] as f32
                                    / 255.0;
                        }
                        let dst = image.get_pixel_mut(x, y);
                        for ch in 0..3 {
                            dst.0[ch] = (src[ch] + dst.0[ch] as f32 * (1.0 - src[3] / 255.0))
                                .clamp(0.0, 255.0) as u8;
                        }
                        dst.0[3] = 255;
                    }
                }
            }
        }
        image
    }
}
fn button(pos: egui::Pos2, pressed: bool) -> Input {
    Input::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}
fn text_position(shapes: &[egui::epaint::ClippedShape], text: &str) -> Option<egui::Pos2> {
    fn find(s: &egui::Shape, text: &str) -> Option<egui::Pos2> {
        match s {
            egui::Shape::Text(t) if t.galley.job.text == text => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            egui::Shape::Vec(v) => v.iter().find_map(|s| find(s, text)),
            _ => None,
        }
    }
    shapes
        .iter()
        .find_map(|s| find(&s.shape, text).filter(|p| s.clip_rect.contains(*p)))
}

#[test]
fn workshop_journal_starts_closed_and_opens_on_injected_events() {
    let size = vec2(1366.0, 768.0);
    let mut h = Harness::new();
    h.app.show_workflow = true;
    h.app.config.keys_mut()[0].steps = vec![Step::Delay { ms: 100 }];
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "Simulation · Aucun test effectué").is_some());
    assert!(text_position(&output.shapes, "Journal").is_some());
    assert!(text_position(&output.shapes, "SIMULATION").is_none());
    h.app.workshop_journal.begin(true);
    h.app.workshop_journal.step(vec![0], "Attendre".into(), None);
    h.app.workshop_journal.step(vec![0], "Attendre".into(), Some(Duration::from_millis(5)));
    h.app.workshop_journal.finish(true, Duration::from_millis(1200), "Macro terminée".into());
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "Simulation · 1 étapes · OK · 1.2 s").is_some());
    assert!(text_position(&output.shapes, "SIMULATION").is_some());
    assert!(text_position(&output.shapes, "1 · Attendre · 5.0 ms").is_some());
}

#[test]
fn workshop_real_run_requires_explicit_confirmation() {
    let size = vec2(1366.0, 768.0);
    let mut h = Harness::new();
    h.app.show_workflow = true;
    h.app.config.keys_mut()[0].steps = vec![Step::Delay { ms: 100 }];
    let output = h.warm(size);
    let button = text_position(&output.shapes, "Tester pour de vrai").unwrap();
    h.click(size, button);
    let output = h.warm(size);
    assert!(h.app.workshop_journal.confirm_real);
    assert!(text_position(&output.shapes, "Ceci va réellement exécuter 1 blocs sur ce PC.").is_some());
    assert_eq!(h.app.workshop_journal.mode, None);
    assert!(!h.app.engine.running.load(Ordering::SeqCst));
    // Merely opening the confirmation never runs the macro.
    assert_eq!(h.app.workshop_journal.mode, None);
}

#[test]
fn real_widgets_assign_catalog_bind_swap_keys_and_undo() {
    let size = vec2(1380.0, 900.0);
    let mut h = Harness::new();
    h.warm(size);
    let source = h
        .app
        .drag_sources
        .iter()
        .find_map(|(r, v)| match v {
            DragItem::Bind(k) if k.label == "Piste suivante" => Some(r.center()),
            _ => None,
        })
        .unwrap();
    let target = h.app.drop_targets[7].0.center();
    h.frame(
        size,
        vec![Input::PointerMoved(source), button(source, true)],
    );
    h.frame(
        size,
        vec![Input::PointerMoved(target), button(target, false)],
    );
    assert_eq!(h.app.config.keys()[7].label, "Piste suivante");
    assert!(!h.app.history.is_empty());
    h.app.undo_edit();
    assert!(h.app.config.keys()[7].steps.is_empty());
    h.warm(size);
    let a = h.app.drop_targets[1].0.center();
    let b = h.app.drop_targets[7].0.center();
    h.frame(size, vec![button(a, true)]);
    h.frame(size, vec![Input::PointerMoved(b), button(b, false)]);
    assert_eq!(h.app.config.keys()[7].label, "Lecture");
    assert!(h.app.config.keys()[1].steps.is_empty());
    h.app.save();
    let loaded = store::load(&h.dir.path().join("config.json")).unwrap();
    assert_eq!(loaded.keys()[7].label, "Lecture");
}
#[test]
fn recorder_button_captures_chord_without_running_macro() {
    let size = vec2(1380.0, 900.0);
    let mut h = Harness::new();
    h.app.config.keys_mut()[6] = catalog::action(0, "Bureau");
    h.app.selected = 6;
    let output = h.warm(size);
    let pos =
        text_position(&output.shapes, "Enregistrer…").expect("Recording button must be visible");
    h.click(size, pos);
    assert!(h.app.recording.is_some());
    h.frame(
        size,
        vec![Input::Key {
            key: egui::Key::S,
            physical_key: Some(egui::Key::S),
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
        }],
    );
    assert!(h.app.recording.is_none());
    assert_eq!(
        h.app.config.keys()[6].steps,
        vec![Step::Hotkey {
            keys: "CTRL+SHIFT+S".into()
        }]
    );
    assert!(!h.app.engine.running.load(Ordering::SeqCst));
    assert!(!h.app.gate.enabled);
}
#[test]
fn physical_navigation_reaches_upload_and_waits_for_matching_ack() {
    let mut h = Harness::new();
    h.app.config.profiles[0].pages.push(crate::model::Page::new(
        "p2".into(),
        "Page 2".into(),
        false,
    ));
    h.app.config.keys_mut()[0] = Key {
        label: "Suivante".into(),
        steps: vec![Step::PageNext],
        ..Key::default()
    };
    h.app.config.profiles[0].pages[0].keys[2].label = "Sur page 2".into();
    h.app.applied = h.app.config.clone();
    let (tx, rx) = mpsc::channel();
    h.app.engine = Engine::new(tx.clone());
    h.app.events = rx;
    let (device, commands) = mpsc::channel();
    h.app.device = device;
    h.app.wanted = true;
    h.app.gate.enabled = true;
    h.app.gate.connected = true;
    let initial = h.app.gate.request_apply();
    h.app.gate.applied(initial);
    tx.send(Event::Key(0, Instant::now())).unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    while h.app.config.page != "p2" && Instant::now() < deadline {
        h.app.events();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(h.app.config.page, "p2");
    assert!(matches!(h.app.gate.decision(), Decision::Blocked(_)));
    let Command::Update(config, revision) = commands.recv_timeout(Duration::from_secs(1)).unwrap()
    else {
        panic!("Page navigation must synchronize the D6");
    };
    assert_eq!(config.keys()[2].label, "Sur page 2");
    tx.send(Event::Applied(revision)).unwrap();
    h.app.events();
    assert_eq!(h.app.applied.page, "p2");
    assert_eq!(h.app.gate.decision(), Decision::Execute);
}
#[test]
fn page_management_preserves_other_pages_and_removes_dangling_folder_keys() {
    let mut h = Harness::new();
    h.app.create_page(false);
    assert_eq!(h.app.config.profiles[0].pages[0].name, "Page 2");
    let page = h.app.config.page.clone();
    h.app.selected = 3;
    h.app.create_page(true);
    let folder = h.app.config.profiles[0]
        .pages
        .iter()
        .find(|p| p.folder)
        .unwrap()
        .id
        .clone();
    h.app.navigate(Step::Folder { id: folder.clone() });
    assert_eq!(h.app.config.page, folder);
    h.app.remove_page(&folder);
    assert!(h.app.config.page.is_empty());
    assert!(h.app.config.profiles[0].keys_at(&page).unwrap()[3]
        .steps
        .is_empty());
    h.app.config.validate().unwrap();
    h.app.create_page(false);
    assert_eq!(
        h.app.config.profiles[0].pages.last().unwrap().name,
        "Page 3"
    );
    h.app.add_profile(false);
    assert!(h.app.config.page.is_empty());
    h.app.config.validate().unwrap();
    h.app.select_profile(0);
    assert!(h.app.config.page.is_empty());
}
#[test]
fn deck_scales_with_window_without_scrolling() {
    let mut widths = Vec::new();
    for size in [vec2(1060.0, 720.0), vec2(1380.0, 900.0), vec2(1920.0, 1200.0)] {
        let mut h = Harness::new();
        h.warm(size);
        assert_eq!(h.app.drop_targets.len(), 15);
        let last = h.app.drop_targets[14].0;
        assert!((last.width() / last.height() - crate::deck_chassis::FACE_SIZE.x / crate::deck_chassis::FACE_SIZE.y).abs() < 0.01 && last.max.y <= size.y, "{size:?}: {last:?}");
        widths.push(h.app.drop_targets[0].0.width());
    }
    assert!(widths[0] < widths[1] && widths[1] < widths[2], "{widths:?}");
}
#[test]
fn all_fifteen_keys_remain_visible_in_both_themes_and_window_sizes() {
    for (size, label, dark) in [
        (vec2(1380.0, 900.0), "preview-dark.png", true),
        (vec2(1060.0, 720.0), "preview-small-dark.png", true),
        (vec2(1380.0, 900.0), "preview-light.png", false),
        (vec2(1060.0, 720.0), "preview-small-light.png", false),
    ] {
        let mut h = Harness::new();
        h.app.preferences.dark = dark;
        h.app.selected = 1;
        theme::apply(&h.ctx, dark);
        let output = h.warm(size);
        assert_eq!(h.app.drop_targets.len(), 15);
        for (r, i) in &h.app.drop_targets {
            assert!(
                r.width() > 47.0 && (r.width() / r.height() - crate::deck_chassis::FACE_SIZE.x / crate::deck_chassis::FACE_SIZE.y).abs() < 0.01,
                "Key {} clipped in {}: {:?}",
                i + 1,
                label,
                r
            );
        }
        if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
            std::fs::create_dir_all(&dir).unwrap();
            h.png(size, output, &std::path::PathBuf::from(dir).join(label));
        }
    }
}

#[test]
fn editor_textures_match_usb_images_with_custom_files_audio_states_and_fallbacks() {
    let mut h = Harness::new();
    let size = vec2(1380.0, 900.0);
    let custom = h.dir.path().join("asymmetric.png");
    let mut image = image::RgbImage::from_pixel(90, 55, image::Rgb([184, 65, 22]));
    for y in 0..20 {
        for x in 0..45 {
            image.put_pixel(x, y, image::Rgb([22, 182, 233]));
        }
    }
    image.save(&custom).unwrap();
    let svg = h.dir.path().join("custom.svg");
    std::fs::write(&svg, "<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'><rect x='10' y='5' width='50' height='30' fill='#22bb77'/></svg>").unwrap();
    h.app.config.keys_mut()[5] = Key {
        label: "Mon image".into(),
        image: custom.to_string_lossy().into_owned(),
        steps: vec![Step::Delay { ms: 1 }],
        ..Key::default()
    };
    h.app.config.keys_mut()[6] = Key {
        label: "SVG".into(),
        image: svg.to_string_lossy().into_owned(),
        steps: vec![Step::Delay { ms: 1 }],
        ..Key::default()
    };
    h.app.config.keys_mut()[7] = crate::audio::tests::key();
    h.app.config.keys_mut()[8] = Key {
        label: "Fichier absent".into(),
        image: h
            .dir
            .path()
            .join("missing.png")
            .to_string_lossy()
            .into_owned(),
        steps: vec![Step::Media {
            command: "mute".into(),
        }],
        ..Key::default()
    };
    h.app.config.keys_mut()[13] = Key {
        label: "Page précédente".into(),
        steps: vec![Step::PagePrevious],
        ..Key::default()
    };
    h.app.config.keys_mut()[14] = Key {
        label: "Page suivante".into(),
        steps: vec![Step::PageNext],
        ..Key::default()
    };
    for (dark, endpoint, name) in [
        (true, "headset", "shared-preview-dark.png"),
        (false, "speakers", "shared-preview-light.png"),
    ] {
        h.app.preferences.dark = dark;
        theme::apply(&h.ctx, dark);
        h.app.audio.output = endpoint.into();
        let output = h.warm(size);
        for index in 0..15 {
            let key = h.app.presented_key(index);
            let texture = h
                .app
                .key_previews
                .texture(&h.ctx, index, &key, h.app.config.image_size, None)
                .unwrap();
            let gui = &h.textures[&texture.id()];
            let (bytes, _) = crate::key_render::for_device(
                &key,
                h.app.config.image_size,
                &crate::key_render::font().unwrap(),
            )
            .unwrap();
            let lcd =
                image::imageops::rotate180(&image::load_from_memory(&bytes).unwrap().into_rgb8());
            assert_eq!(gui.size, [lcd.width() as usize, lcd.height() as usize]);
            for (pixel, expected) in gui.pixels.iter().zip(lcd.pixels()) {
                assert_eq!(
                    pixel.to_array(),
                    [expected[0], expected[1], expected[2], 255]
                );
            }
        }
        if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
            std::fs::create_dir_all(&dir).unwrap();
            h.png(size, output, &PathBuf::from(dir).join(name));
        }
    }
}

#[test]
fn lcd_preview_cache_refreshes_labels_colors_icons_and_changed_image_files() {
    let mut h = Harness::new();
    let size = vec2(1380.0, 900.0);
    let file = h.dir.path().join("mutable.png");
    image::RgbImage::from_pixel(10, 10, image::Rgb([220, 30, 50]))
        .save(&file)
        .unwrap();
    h.app.config.keys_mut()[5].image = file.to_string_lossy().into_owned();
    h.warm(size);
    let key = h.app.presented_key(5);
    let old = h
        .app
        .key_previews
        .texture(&h.ctx, 5, &key, 126, None)
        .unwrap()
        .id();
    image::RgbImage::from_pixel(49, 23, image::Rgb([12, 220, 80]))
        .save(&file)
        .unwrap();
    let new = h
        .app
        .key_previews
        .texture(&h.ctx, 5, &key, 126, None)
        .unwrap()
        .id();
    assert_ne!(old, new);
    assert_eq!(
        new,
        h.app
            .key_previews
            .texture(&h.ctx, 5, &key, 126, None)
            .unwrap()
            .id()
    );
    let mut changed = key.clone();
    changed.label = "Nouveau titre".into();
    let label = h
        .app
        .key_previews
        .texture(&h.ctx, 5, &changed, 126, None)
        .unwrap()
        .id();
    assert_ne!(new, label);
    changed.color = [12, 34, 56];
    let color = h
        .app
        .key_previews
        .texture(&h.ctx, 5, &changed, 126, None)
        .unwrap()
        .id();
    assert_ne!(label, color);
    changed.background = false;
    let black = h
        .app
        .key_previews
        .texture(&h.ctx, 5, &changed, 126, None)
        .unwrap()
        .id();
    assert_ne!(color, black);
    changed.image.clear();
    changed.steps = vec![Step::Media {
        command: "play_pause".into(),
    }];
    let play = h
        .app
        .key_previews
        .texture(&h.ctx, 5, &changed, 126, None)
        .unwrap()
        .id();
    assert_ne!(black, play);
    changed.steps = vec![Step::Media {
        command: "mute".into(),
    }];
    assert_ne!(
        play,
        h.app
            .key_previews
            .texture(&h.ctx, 5, &changed, 126, None)
            .unwrap()
            .id()
    );
}
#[test]
fn imports_real_export_when_reference_is_provided() {
    if let Some(path) = std::env::var_os("DECKFORGE_ELGATO_REFERENCE") {
        let report = crate::elgato::load(&PathBuf::from(path)).unwrap();
        assert!(report.converted > 0);
        if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
            std::fs::create_dir_all(&dir).unwrap();
            let result = serde_json::json!({"converted":report.converted,"pages":report.profile.pages.len()+1,"icons":report.icons.len(),"warnings":report.warnings});
            std::fs::write(
                PathBuf::from(dir).join("elgato-import-check.json"),
                serde_json::to_vec_pretty(&result).unwrap(),
            )
            .unwrap();
        }
    }
}

#[test]
fn incomplete_shortcut_is_saved_as_draft_while_youtube_reaches_usb() {
    let mut h = Harness::new();
    let size = vec2(1380.0, 900.0);
    let (tx, rx) = mpsc::channel();
    h.app.device = tx;
    h.app.wanted = true;
    h.app.gate.connected = true;
    h.app.gate.enabled = true;
    h.app.config.keys_mut()[0].steps = vec![Step::Hotkey {
        keys: String::new(),
    }];
    let youtube = catalog::presets()
        .into_iter()
        .find(|p| p.key.label == "YouTube")
        .unwrap()
        .key;
    h.app.assign(DragItem::Bind(youtube), 5);
    h.app
        .assign(DragItem::Action(catalog::action(5, "Bureau")), 6);
    h.app.last_edit = Instant::now() - Duration::from_secs(2);
    h.frame(size, vec![]);
    let saved = store::load(&h.app.path).unwrap();
    assert!(matches!(&saved.keys()[0].steps[0],Step::Hotkey{keys} if keys.is_empty()));
    assert_eq!(saved.keys()[5].label, "YouTube");
    assert!(!h.app.gate.dirty);
    let Command::Update(sent, revision) = rx.recv_timeout(Duration::from_secs(1)).unwrap() else {
        panic!("USB update expected")
    };
    assert!(sent.keys()[0].steps.is_empty());
    assert_eq!(sent.keys()[5].label, "YouTube");
    assert_eq!(sent.keys()[6].steps, vec![Step::Delay { ms: 500 }]);
    sent.validate().unwrap();
    h.app.gate.applied(revision);
    assert_eq!(h.app.gate.decision(), Decision::Execute);
    assert!(saved.binding_errors()[0].contains("touche 01"));
}

#[test]
fn nested_blocks_and_recorder_survive_restart_and_can_be_edited_in_workflow() {
    let size = vec2(1380.0, 900.0);
    let mut h = Harness::new();
    h.app.config.keys_mut()[0].steps = vec![Step::Repeat {
        count: 2,
        steps: vec![Step::IfVariable {
            name: "mode".into(),
            value: "travail".into(),
            then_steps: vec![Step::Hotkey {
                keys: "CTRL+C".into(),
            }],
            else_steps: vec![],
        }],
    }];
    h.app.show_workflow = true;
    h.warm(size);
    h.app.recording = Some((0, String::new(), 0, vec![0, 0, 0, 0, 0], false));
    h.frame(
        size,
        vec![Input::Key {
            key: egui::Key::S,
            physical_key: Some(egui::Key::S),
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
        }],
    );
    h.app.save();
    let c = store::load(&h.app.path).unwrap();
    assert!(serde_json::to_string(&c).unwrap().contains("CTRL+SHIFT+S"));
    c.validate().unwrap();
    assert!(!h.app.engine.running.load(Ordering::SeqCst));
}

#[test]
fn audio_feedback_updates_lcd_presentation_without_overwriting_binding() {
    let mut h = Harness::new();
    let (tx, rx) = mpsc::channel();
    h.app.device = tx;
    h.app.wanted = true;
    let mut key = catalog::action(13, "Bureau");
    if let Step::AudioSwitch { first, second, .. } = &mut key.steps[0] {
        *first = "headset".into();
        *second = "speakers".into();
    }
    h.app.config.keys_mut()[5] = key;
    h.app.audio.output = "headset".into();
    h.app.upload(false);
    let Command::Update(c, _) = rx.recv().unwrap() else {
        panic!()
    };
    assert_eq!(c.keys()[5].label, "Casque");
    assert_eq!(c.keys()[5].display_kind, Some(26));
    h.app.audio.output = "speakers".into();
    h.app.upload(false);
    let Command::Update(c, _) = rx.recv().unwrap() else {
        panic!()
    };
    assert_eq!(c.keys()[5].label, "Enceintes");
    assert_eq!(c.keys()[5].display_kind, Some(27));
    assert_eq!(
        h.app.config.keys()[5].label,
        "Basculer le périphérique audio"
    );
}
#[test]
fn background_tick_processes_physical_events_and_saves_without_drawing_a_frame() {
    let mut h = Harness::new();
    let (tx, rx) = mpsc::channel();
    h.app.engine = Engine::new(tx.clone());
    h.app.events = rx;
    h.app.config.keys_mut()[6] = catalog::action(5, "Bureau");
    h.app.applied = h.app.config.clone();
    h.app.gate.enabled = true;
    h.app.gate.connected = true;
    let revision = h.app.gate.request_apply();
    h.app.gate.applied(revision);
    tx.send(Event::Key(6, Instant::now())).unwrap();
    h.app.background_tick();
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        h.app.background_tick();
        if h.app.logs.iter().any(|s| s.contains("Macro terminée")) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(h.app.logs.iter().any(|s| s.contains("EXÉCUTION")));
    assert!(h.app.logs.iter().any(|s| s.contains("Macro terminée")));
    h.app.config.keys_mut()[6].label = "Attente locale".into();
    h.app.edited();
    h.app.last_edit = Instant::now() - Duration::from_secs(2);
    h.app.background_tick();
    assert_eq!(
        store::load(&h.app.path).unwrap().keys()[6].label,
        "Attente locale"
    );
}

#[test]
fn workshop_chat_opens_previews_inserts_and_undoes_without_running() {
    let size = vec2(1366.0, 768.0);
    let mut h = Harness::new();
    h.app.show_workflow = true;
    h.app.config.keys_mut()[0].steps.clear();
    let output = h.warm(size);
    assert!(!h.app.workshop_chat.open);
    assert!(text_position(&output.shapes, "Assistant · Atelier").is_none());
    let pos = text_position(&output.shapes, "Assistant").expect("assistant launcher");
    h.click(size, pos);
    let output = h.warm(size);
    assert!(h.app.workshop_chat.open);
    assert!(text_position(&output.shapes, "Assistant · Atelier").is_some());
    h.app.workshop_chat.input = "attends 2 s".into();
    let pos = text_position(&output.shapes, "Envoyer").expect("send button");
    h.click(size, pos);
    let output = h.warm(size);
    assert!(matches!(h.app.workshop_chat.pending.as_ref().unwrap().proposal.steps.as_slice(), [Step::Delay { ms: 2000 }]));
    assert!(h.app.config.keys()[0].steps.is_empty());
    let pos = text_position(&output.shapes, "Ajouter").expect("add button");
    h.click(size, pos);
    assert!(matches!(h.app.config.keys()[0].steps.as_slice(), [Step::Delay { ms: 2000 }]));
    assert!(!h.app.engine.running.load(Ordering::SeqCst));
    assert!(h.app.workshop_journal.lines.is_empty());
    h.app.undo_edit();
    assert!(h.app.config.keys()[0].steps.is_empty());
}

#[test]
fn workshop_chat_pending_becomes_stale_on_key_change() {
    let mut h = Harness::new();
    let size = vec2(1366.0, 768.0);
    h.app.show_workflow = true;
    h.app.config.keys_mut()[1].steps.clear();
    h.app.workshop_chat.open = true;
    h.warm(size);
    h.app.workshop_chat_send("attends 2 s".into(), None);
    h.app.selected = 1;
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "Proposition périmée : touche, page ou profil changé.").is_some());
    assert!(h.app.config.keys()[1].steps.is_empty());
    assert!(!h.app.engine.running.load(Ordering::SeqCst));
}

#[test]
fn workshop_selection_shortcuts_duplicate_delete_and_undo() {
    let size = vec2(1366.0, 768.0);
    let mut h = Harness::new();
    h.app.show_workflow = true;
    h.app.preferences.workshop_advanced = false;
    h.app.config.keys_mut()[0].steps = vec![Step::Delay { ms: 120 }];
    let output = h.warm(size);
    let title = output.shapes.iter().filter_map(|shape| if let egui::Shape::Text(t) = &shape.shape {
        (t.galley.job.text.contains("Attendre 120 ms")).then_some(t.galley.job.text.clone())
    } else { None }).next().expect("step card title");
    let pos = text_position(&output.shapes, &title).unwrap();
    h.click(size, pos);
    h.warm(size);
    assert_eq!(h.ctx.data(|d| d.get_temp::<Vec<usize>>(egui::Id::new("workshop_path"))), Some(vec![0]));
    let key = |key, modifiers| Input::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers };
    h.frame(size, vec![key(egui::Key::D, Modifiers { ctrl: true, ..Default::default() })]);
    assert_eq!(h.app.config.keys()[0].steps.len(), 2);
    h.frame(size, vec![key(egui::Key::Delete, Modifiers::NONE)]);
    assert_eq!(h.app.config.keys()[0].steps.len(), 1);
    h.app.undo_edit();
    assert_eq!(h.app.config.keys()[0].steps.len(), 2);
}

fn fully_visible(shapes: &[egui::epaint::ClippedShape], text: &str) -> bool {
    fn find(s: &egui::Shape, text: &str) -> Option<Rect> {
        match s {
            egui::Shape::Text(t) if t.galley.job.text == text => Some(Rect::from_min_size(t.pos, t.galley.size())),
            egui::Shape::Vec(v) => v.iter().find_map(|s| find(s, text)),
            _ => None,
        }
    }
    shapes.iter().any(|s| find(&s.shape, text).is_some_and(|r| s.clip_rect.expand(0.5).contains_rect(r)))
}

/// Laisse le temps au décodage des GIF (fil d'arrière-plan) puis renvoie une image stable.
fn settle(h: &mut Harness, size: Vec2) -> egui::FullOutput {
    for _ in 0..30 {
        h.frame(size, vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    h.warm(size)
}

fn demo_harness() -> Harness {
    let mut h = Harness::new();
    h.app.add_demo_profile();
    h
}

#[test]
fn demo_profile_is_added_beside_user_profiles_and_resets_by_name() {
    let mut h = Harness::new();
    h.app.config.profiles[0].keys[3].label = "Mon réglage".into();
    let before = serde_json::to_value(&h.app.config.profiles).unwrap();
    h.app.add_demo_profile();
    h.app.add_demo_profile();
    let after = serde_json::to_value(&h.app.config.profiles).unwrap();
    assert_eq!(after.as_array().unwrap().len(), before.as_array().unwrap().len() + 1, "un seul profil de démonstration");
    assert_eq!(after.as_array().unwrap()[..before.as_array().unwrap().len()], before.as_array().unwrap()[..], "profils de l'utilisateur intacts");
    assert_eq!(h.app.config.profiles[h.app.config.active].name, crate::demo_profile::NAME);
    assert!(h.app.config.keys().iter().take(15).all(|k| !k.steps.is_empty()));
    let size = vec2(1380.0, 900.0);
    settle(&mut h, size);
    assert_eq!(h.app.drop_targets.len(), 15);
    h.app.navigate(Step::PageNext);
    h.warm(size);
    assert_eq!(h.app.config.page, "demo-page-2");
}

/// Captures du README : vraie interface rendue hors écran à 2560 px de large (1600 × 1000 points à 160 %).
#[test]
fn readme_captures_from_the_demo_profile() {
    let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) else { return };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let size = vec2(1600.0, 1000.0);
    let shot = |h: &mut Harness, name: &str| {
        h.ctx.set_pixels_per_point(1.6);
        let out = settle(h, size);
        let shapes = out.shapes.clone();
        h.png(size, out, &dir.join(name));
        shapes
    };
    let mut h = demo_harness();
    h.app.selected = 8;
    shot(&mut h, "novydeck-main.png");
    let mut h = demo_harness();
    h.app.show_workflow = true;
    h.app.preferences.workshop_advanced = false;
    h.app.selected = 0;
    h.warm(size); // l'atelier oublie la sélection quand la touche ciblée change
    h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("workshop_path"), vec![1_usize]));
    let shapes = shot(&mut h, "novydeck-block-editor.png");
    assert!(fully_visible(&shapes, "Enregistrer…"), "bouton d'enregistrement du raccourci coupé");
    let mut h = demo_harness();
    h.app.show_workflow = true;
    h.app.selected = 5;
    h.app.workshop_chat.open = true;
    h.app.workshop_chat_send("ouvre le navigateur brave s'il est pas ouvert et si il deja ouvert ouvre un nouvel onglet".into(), None);
    assert!(matches!(h.app.workshop_chat.pending.as_ref().expect("proposition réelle").proposal.steps.as_slice(), [Step::IfCondition { .. }]));
    shot(&mut h, "novydeck-assistant.png");
    let mut h = demo_harness();
    let photo = PathBuf::from(&h.app.config.keys()[8].image);
    let page = h.app.config.page.clone();
    h.app.open_crop_editor(h.app.config.active, page, 8, photo, 126);
    assert!(h.app.crop_session.is_some(), "recadrage ouvert");
    shot(&mut h, "novydeck-media-crop.png");
}

/// Images clés de la vidéo de présentation (1920 × 1080) : uniquement des états réels de l'interface.
#[test]
#[ignore = "produit les images de la vidéo ; DECKFORGE_VIDEO_OUTPUT requis"]
fn demo_video_keyframes() {
    let dir = PathBuf::from(std::env::var_os("NOVYDECK_VIDEO_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_VIDEO_OUTPUT")).expect("DECKFORGE_VIDEO_OUTPUT"));
    std::fs::create_dir_all(&dir).unwrap();
    let size = vec2(1280.0, 720.0);
    let shot = |h: &mut Harness, name: &str| {
        h.ctx.set_pixels_per_point(1.5);
        let out = settle(h, size);
        h.png(size, out, &dir.join(format!("{name}.png")));
    };
    let mut h = demo_harness();
    // Écrans LCD réels des 15 touches (moteur `key_render`, 252 px) pour la séquence 3D.
    let keys_dir = dir.join("keys");
    std::fs::create_dir_all(&keys_dir).unwrap();
    let font = crate::key_render::font().unwrap();
    for (i, key) in h.app.config.keys().iter().take(15).enumerate() {
        crate::key_render::preview(key, 252).unwrap().save(keys_dir.join(format!("key-{i:02}.png"))).unwrap();
        for (f, frame) in crate::key_render::animated(key, 252, &font).unwrap().iter().enumerate() {
            let upright = image::imageops::rotate180(&image::load_from_memory(&frame.jpeg).unwrap().into_rgb8());
            upright.save(keys_dir.join(format!("key-{i:02}-f{f:02}.png"))).unwrap();
        }
    }
    shot(&mut h, "01-main");
    h.app.navigate(Step::PageNext);
    shot(&mut h, "02-page-2");
    h.app.navigate(Step::PagePrevious);
    h.app.selected = 8;
    shot(&mut h, "03-photo-key");
    h.app.selected = 0;
    h.app.show_workflow = true;
    h.app.preferences.workshop_advanced = false;
    h.warm(size);
    h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("workshop_path"), vec![1_usize]));
    shot(&mut h, "04-workshop");
    let path = crate::workshop_ops::apply(&mut h.app.config.keys_mut()[0].steps,
        crate::workshop_ops::Edit::Insert(vec![4], Step::Delay { ms: 500 })).expect("bloc inséré");
    h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("workshop_path"), path.clone()));
    shot(&mut h, "05-block-added");
    if let Some(Step::Delay { ms }) = h.app.config.keys_mut()[0].steps.get_mut(4) { *ms = 1200; }
    shot(&mut h, "06-block-edited");
    h.app.workshop_chat.open = true;
    h.app.workshop_chat_send("ouvre le navigateur brave s'il est pas ouvert et si il deja ouvert ouvre un nouvel onglet".into(), None);
    shot(&mut h, "07-assistant");
    let mut h = demo_harness();
    let photo = PathBuf::from(&h.app.config.keys()[8].image);
    let page = h.app.config.page.clone();
    h.app.open_crop_editor(h.app.config.active, page, 8, photo, 126);
    shot(&mut h, "08-media-crop");
}

#[test]
fn key_media_is_masked_by_the_rounded_lcd_corners() {
    // Fond plein jusqu'aux bords : aucun angle carré ne doit dépasser de l'arrondi de la touche.
    let mut failures = Vec::new();
    for dpi in [1.0_f32, 1.5, 2.0] {
        let mut h = Harness::new();
        h.ctx.set_pixels_per_point(dpi);
        let size = vec2(1380.0, 900.0);
        h.app.config.keys_mut()[0] = Key { label: String::new(), color: [230, 20, 20], background: true, steps: vec![Step::Delay { ms: 1 }], ..Default::default() };
        h.app.selected = 0;
        let out = h.warm(size);
        let face = h.app.drop_targets.iter().find(|(_, k)| *k == 0).unwrap().0;
        let lcd = crate::deck_chassis::lcd(face, face.width() / crate::deck_chassis::FACE_SIZE.x);
        let img = h.raster(size, out);
        if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
            std::fs::create_dir_all(&dir).unwrap();
            let r = Rect::from_center_size(face.center(), face.size() * 1.15);
            let crop = image::imageops::crop_imm(&img, (r.left() * dpi) as u32, (r.top() * dpi) as u32, (r.width() * dpi) as u32, (r.height() * dpi) as u32).to_image();
            crop.save(PathBuf::from(dir).join(format!("key-corners-{}.png", (dpi * 100.0) as u32))).unwrap();
        }
        let px = |p: egui::Pos2| *img.get_pixel((p.x * dpi) as u32, (p.y * dpi) as u32);
        let red = |c: image::Rgba<u8>| c.0[0] > 150 && c.0[1] < 90 && c.0[2] < 90;
        if !(red(px(lcd.left_center() + vec2(2.0, 0.0))) && red(px(lcd.center_top() + vec2(0.0, 2.0)))) { failures.push(format!("média absent des bords à {dpi}")); }
        for corner in [lcd.left_top() + vec2(1.0, 1.0), lcd.right_top() + vec2(-1.0, 1.0), lcd.left_bottom() + vec2(1.0, -1.0), lcd.right_bottom() + vec2(-1.0, -1.0)] {
            if red(px(corner)) { failures.push(format!("angle carré du média visible à {dpi} : {corner:?}")); }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn plugin_header_keeps_remove_button_visible_with_long_plugin_ids() {
    for size in [vec2(1060.0, 720.0), vec2(1366.0, 768.0)] {
        let mut h = Harness::new();
        h.app.selected = 6;
        h.app.config.keys_mut()[6].plugin = Some(crate::model::PluginBinding {
            plugin: "com.geekyeggo.goveecontroller".into(),
            action: "com.geekyeggo.goveecontroller.turnonoff".into(),
            settings: serde_json::json!({}),
            state: 0,
        });
        h.app.plugin_errors.insert("com.geekyeggo.goveecontroller".into(), "Commande plugin expirée sans enregistrement WebSocket".into());
        let output = h.warm(size);
        assert!(fully_visible(&output.shapes, "Retirer le plugin"), "bouton hors champ à {size:?}");
        if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
            let snapshot = h.warm(size);
            h.png(size, snapshot, &PathBuf::from(dir).join(format!("plugin-header-{}.png", size.x as u32)));
        }
    }
}

#[test]
fn workshop_simple_mode_click_on_nested_block_opens_its_properties() {
    let size = vec2(1366.0, 768.0);
    let mut h = Harness::new();
    h.app.show_workflow = true;
    h.app.preferences.workshop_advanced = false;
    h.app.config.keys_mut()[0].steps = vec![Step::IfCondition {
        condition: crate::conditions::Condition::AppRunning { exe: "brave.exe".into() },
        then_steps: vec![Step::Hotkey { keys: "CTRL+T".into() }],
        else_steps: vec![Step::Launch { program: "brave.exe".into(), args: vec![], focus_if_running: false }],
    }];
    // Fenêtre compacte : bibliothèque de blocs ouverte, puis assistant ouvert.
    for chat in [false, true] {
        h.ctx.data_mut(|d| { d.insert_temp(egui::Id::new("workshop_palette_open"), true); d.remove::<Vec<usize>>(egui::Id::new("workshop_path")); });
        h.app.workshop_chat.open = chat;
        let output = h.warm(size);
        let card = text_position(&output.shapes, "Lancer une application · Lancer brave.exe").expect("carte imbriquée visible");
        h.click(size, card);
        let output = h.warm(size);
        assert_eq!(h.ctx.data(|d| d.get_temp::<Vec<usize>>(egui::Id::new("workshop_path"))), Some(vec![0, 1, 0]));
        assert!(text_position(&output.shapes, "Propriétés du bloc").is_some(), "propriétés du bloc imbriqué (assistant {chat})");
        assert!(text_position(&output.shapes, "Programme").is_some() || text_position(&output.shapes, "brave.exe").is_some(), "champ modifiable");
        if let (false, Some(dir)) = (chat, std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT"))) {
            let snapshot = h.warm(size);
            h.png(size, snapshot, &PathBuf::from(dir).join("atelier-simple-proprietes.png"));
        }
    }
}

#[test]
fn workshop_simple_layout_keeps_canvas_visible_at_both_sizes() {
    for size in [vec2(1366.0, 768.0), vec2(1920.0, 1080.0)] {
        let mut h = Harness::new();
        h.app.show_workflow = true;
        h.app.preferences.workshop_advanced = false;
        h.app.config.keys_mut()[0].steps = vec![Step::Repeat {
            count: 2, steps: vec![Step::IfVariable {
                name: "mode".into(), value: "on".into(), then_steps: vec![Step::Delay { ms: 100 }], else_steps: vec![],
            }],
        }];
        let output = h.warm(size);
        assert!(text_position(&output.shapes, "Construis ton automatisation").is_some());
        assert!(text_position(&output.shapes, "Propriétés du bloc").is_none());
        assert!(text_position(&output.shapes, "RÉPÉTER").is_some());
        assert!(text_position(&output.shapes, "ALORS").is_some());
        assert!(text_position(&output.shapes, "SINON").is_some());
        assert_eq!(h.ctx.data(|d| d.get_temp::<(usize, String, usize)>(egui::Id::new("workshop_context"))), Some((0, String::new(), 0)));
        h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("workshop_path"), vec![0_usize, 0, 0]));
        assert_eq!(h.ctx.data(|d| d.get_temp::<Vec<usize>>(egui::Id::new("workshop_path"))), Some(vec![0, 0, 0]));
        // A new egui Window is invisible during its first sizing pass.
        let output = h.warm(size);
        assert!(text_position(&output.shapes, "Propriétés du bloc").is_some(), "path={:?}, shown={:?}, texts={:?}", h.ctx.data(|d| d.get_temp::<Vec<usize>>(egui::Id::new("workshop_path"))), h.app.show_workflow, output.shapes.iter().filter_map(|shape| if let egui::Shape::Text(t) = &shape.shape {Some(t.galley.job.text.clone())} else {None}).collect::<Vec<_>>());
        assert!(text_position(&output.shapes, "Construis ton automatisation").is_some());
        h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("workshop_palette_open"), true));
        let output = h.warm(size);
        assert!(text_position(&output.shapes, "BIBLIOTHÈQUE DE BLOCS").is_some());
        h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("workshop_palette_open"), false));
        h.app.config.keys_mut()[1].steps.clear();
        h.app.selected = 1;
        let output = h.warm(size);
        assert!(text_position(&output.shapes, "Propriétés du bloc").is_none());
        assert!(text_position(&output.shapes, "Ajoute ton premier bloc").is_some());
        h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("workshop_palette_open"), true));
        h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("workshop_search"), "aucun-bloc-xyz".to_owned()));
        let output = h.warm(size);
        assert!(text_position(&output.shapes, "Aucun bloc trouvé. Essaie un autre mot-clé.").is_some());
    }
}

#[test]
fn workflow_preview_shows_conditional_blocks() {
    let mut h = Harness::new();
    let size = vec2(1380.0, 900.0);
    h.app.config.keys_mut()[0] = Key {
        label: "Mode de travail".into(),
        steps: vec![
            Step::SetVariable {
                name: "mode".into(),
                value: "travail".into(),
                    persistent: false,
                },
            Step::IfVariable {
                name: "mode".into(),
                value: "travail".into(),
                then_steps: vec![Step::Repeat {
                    count: 2,
                    steps: vec![Step::Delay { ms: 250 }],
                }],
                else_steps: vec![],
            },
        ],
        ..Key::default()
    };
    h.app.show_workflow = true;
    h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("workshop_palette_open"), true));
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "Construis ton automatisation").is_some());
    assert!(text_position(&output.shapes, "BIBLIOTHÈQUE DE BLOCS").is_some());
    assert!(text_position(&output.shapes, "Clavier").is_some());
    if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
        h.png(size, output, &PathBuf::from(dir).join("workflow.png"));
    }
}

#[test]
fn gallery_click_retargets_to_current_selected_key() {
    let mut h = Harness::new();
    let size = vec2(1380.0, 900.0);
    let source = h.dir.path().join("demo.png");
    image::RgbImage::from_pixel(8, 8, image::Rgb([30, 150, 220]))
        .save(&source)
        .unwrap();
    h.app.selected = 6;
    h.app.show_assets = true;
    h.app.icon_target = Some(icon_ui::Target {
        profile: 0,
        page: String::new(),
        key: 6,
        destination: icon_ui::Destination::Key,
    });
    h.app.available_icons = vec![crate::assets::Icon {
        pack: "Tests locaux".into(),
        name: "Demo".into(),
        path: source.clone(),
    }];
    let output = h.warm(size);
    let pos = text_position(&output.shapes, "Demo").expect("Gallery tile visible");
    h.app.selected = 7;
    h.click(size, pos);
    let output = h.warm(size);
    assert_eq!(h.app.icon_selected.as_deref(), Some(source.as_path()));
    assert!(h.app.config.keys()[6].image.is_empty());
    assert!(!h.app.config.keys()[7].image.is_empty());
    assert!(output
        .shapes
        .iter()
        .any(|s| matches!(&s.shape,egui::Shape::Circle(c) if c.radius==8.0 && c.fill==accent())));
    assert!(h.app.icon_notice.contains("appliquée"));
    assert!(h.app.icon_notice.contains("08"));
    if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
        h.png(
            size,
            output,
            &PathBuf::from(dir).join("gallery-selected.png"),
        );
    }
}

#[test]
fn audio_state_gallery_assigns_only_the_selected_state_and_rejects_stale_blocks() {
    let mut h = Harness::new();
    h.app.config.keys_mut()[5] = crate::audio::tests::key();
    h.app.selected = 5;
    let target = icon_ui::Target {
        profile: 0,
        page: String::new(),
        key: 5,
        destination: icon_ui::Destination::Audio {
            path: vec![0],
            state: 1,
        },
    };
    h.app.icon_target = Some(target.clone());
    h.app
        .choose_icon("builtin:microphone", "Microphone")
        .unwrap();
    let Step::AudioSwitch {
        first_icon,
        second_icon,
        ..
    } = &h.app.config.keys()[5].steps[0]
    else {
        panic!()
    };
    assert!(first_icon.is_empty());
    assert_eq!(second_icon, "builtin:microphone");
    h.app.save();
    let loaded = store::load(&h.app.path).unwrap();
    assert_eq!(target.image(&loaded).unwrap(), "builtin:microphone");
    h.app.config.keys_mut()[5].steps[0] = Step::Delay { ms: 100 };
    assert!(h.app.choose_icon("builtin:speakers", "Enceintes").is_err());
    assert_eq!(h.app.config.keys()[5].steps, vec![Step::Delay { ms: 100 }]);
}

#[test]
fn appearance_inspector_sizes_have_correct_units_and_icon_slider_can_scroll_into_view() {
    let size = vec2(1380.0, 1050.0);
    let mut h = Harness::new();
    h.app.config.keys_mut()[0].title_size = 14;
    h.app.config.keys_mut()[0].icon_scale = 100;
    h.ctx.data_mut(|d| d.insert_persisted(egui::Id::new("inspector_tab"), 1u8));
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "14 px").is_some(), "title size is measured in pixels");
    assert!(text_position(&output.shapes, "Taille du titre").is_some());
    let origin = text_position(&output.shapes, "Titre sur la touche").unwrap();
    let area = pos2(origin.x + 85.0, origin.y + 90.0);
    let mut visible = text_position(&output.shapes, "Taille de l’icône").is_some()
        && text_position(&output.shapes, "100 %").is_some();
    for _ in 0..8 {
        if visible { break; }
        let output = h.frame(size, vec![Input::PointerMoved(area), Input::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -90.0), modifiers: Modifiers::NONE }]);
        visible = text_position(&output.shapes, "Taille de l’icône").is_some()
            && text_position(&output.shapes, "100 %").is_some();
    }
    assert!(visible, "icon size control and percentage must be reachable by scrolling the appearance column");
}

#[test]
fn both_audio_state_previews_are_visible_in_the_compact_inspector() {
    for (size, name) in [
        (vec2(1380.0, 900.0), "audio-options.png"),
        (vec2(1060.0, 720.0), "audio-options-small.png"),
    ] {
        let mut h = Harness::new();
        h.app.config.keys_mut()[5] = crate::audio::tests::key();
        h.app.selected = 5;
        h.app.audio = crate::audio::Snapshot {
            output: "headset".into(),
            devices: vec![
                crate::audio::Endpoint {
                    id: "headset".into(),
                    name: "Casque de test".into(),
                    input: false,
                },
                crate::audio::Endpoint {
                    id: "speakers".into(),
                    name: "Enceintes de test".into(),
                    input: false,
                },
            ],
            ..Default::default()
        };
        h.ctx.data_mut(|d| d.insert_persisted(egui::Id::new("inspector_tab"), 1u8));
        let output = h.warm(size);
        assert!(text_position(&output.shapes, "État 1 · Actif").is_some());
        assert!(text_position(&output.shapes, "État 2").is_some());
        let resets = output
            .shapes
            .iter()
            .filter_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    let center = t.pos + t.galley.size() * 0.5;
                    if t.galley.job.text == "Par défaut" && s.clip_rect.contains(center) {
                        Some(center)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .count();
        assert_eq!(
            resets, 2,
            "Both states' icon controls must be visible without scrolling"
        );
        if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
            h.png(size, output, &PathBuf::from(dir).join(name));
        }
    }
}

#[test]
fn ai_request_carries_the_selected_destination_and_does_not_start_a_model() {
    let mut h = Harness::new();
    h.app.selected = 8;
    h.app.assistant_request = "Créer un bouton casque / enceintes".into();
    let prompt = h.app.automation_prompt();
    assert!(prompt.contains("\"key\":9"));
    assert!(prompt.contains("deckforge_get_audio_devices"));
    assert!(prompt.contains("deckforge_configure_key"));
    assert!(prompt.contains("N’exécute aucune macro"));
    assert!(h.app.proposals.pending.is_empty());
    assert!(!h.app.engine.running.load(Ordering::SeqCst));
}

#[test]
fn mcp_stdio_can_propose_real_audio_blocks_for_review_in_the_headless_app() {
    let Some(binary) = std::env::var_os("DECKFORGE_MCP_EXE") else {
        return;
    };
    use serde_json::{json, Value};
    use std::{
        io::{BufRead, BufReader, Write},
        os::windows::process::CommandExt,
        process::{Command, Stdio},
    };
    let mut h = Harness::new();
    let descriptor = h.dir.path().join("test-connection.json");
    h.app.assistant = Some(crate::mcp_server::Server::new(descriptor.clone()).unwrap());
    h.app.assistant.as_ref().unwrap().set_enabled(true);
    h.app.audio = crate::audio::Snapshot {
        output: "headset".into(),
        devices: vec![
            crate::audio::Endpoint {
                id: "headset".into(),
                name: "Casque de test".into(),
                input: false,
            },
            crate::audio::Endpoint {
                id: "speakers".into(),
                name: "Enceintes de test".into(),
                input: false,
            },
        ],
        ..Default::default()
    };
    h.app.available_icons = vec![crate::assets::Icon {
        pack: "Fixture".into(),
        name: "Fixture".into(),
        path: h.dir.path().join("fixture.png"),
    }];
    let client = std::thread::spawn(move || {
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = Child(
            Command::new(binary)
                .arg(descriptor)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .creation_flags(0x08000000)
                .spawn()
                .unwrap(),
        );
        let mut input = child.0.stdin.take().unwrap();
        let mut output = BufReader::new(child.0.stdout.take().unwrap());
        let mut rpc = |value: Value| {
            writeln!(input, "{value}").unwrap();
            input.flush().unwrap();
            let mut line = String::new();
            output.read_line(&mut line).unwrap();
            serde_json::from_str::<Value>(&line).unwrap()
        };
        assert!(rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}))["result"].is_object());
        let list = rpc(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 11);
        let endpoints = rpc(
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"deckforge_get_audio_devices","arguments":{}}}),
        );
        assert_eq!(
            endpoints["result"]["structuredContent"]["devices"][0]["id"],
            "headset"
        );
        let icons = rpc(
            json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"deckforge_get_icons","arguments":{"query":"Fixture","limit":1}}}),
        );
        assert_eq!(
            icons["result"]["structuredContent"]["audio_defaults"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            icons["result"]["structuredContent"]["icons"][0]["name"],
            "Fixture"
        );
        let proposal = rpc(
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"deckforge_configure_key","arguments":{"profile":"Bureau","key":8,"label":"Audio créé par IA","steps":[{"type":"repeat","count":2,"steps":[{"type":"delay","ms":10}]},{"type":"audio_switch","first":"headset","second":"speakers","input":false,"first_icon":"builtin:headphones","second_icon":"builtin:speakers"}]}}}),
        );
        assert_eq!(proposal["result"]["isError"], false);
        let id = proposal["result"]["structuredContent"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        drop(rpc);
        drop(input);
        let _ = child.0.wait();
        id
    });
    let deadline = Instant::now() + Duration::from_secs(12);
    while !client.is_finished() && Instant::now() < deadline {
        h.app.background_tick();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(client.is_finished(), "MCP request timed out");
    let id = client.join().unwrap();
    assert!(h.app.config.keys()[7].steps.is_empty());
    assert_eq!(h.app.proposals.pending.len(), 1);
    assert!(!h.app.engine.running.load(Ordering::SeqCst));
    let size = vec2(1380.0, 900.0);
    let output = h.warm(size);
    let pos = text_position(&output.shapes, "Valider cette proposition").unwrap();
    h.click(size, pos);
    assert_eq!(h.app.config.keys()[7].label, "Audio créé par IA");
    assert!(matches!(
        h.app.config.keys()[7].steps[0],
        Step::Repeat { count: 2, .. }
    ));
    assert!(!h.app.engine.running.load(Ordering::SeqCst));
    assert_eq!(
        h.app.proposals.status(&json!({"id":id})).unwrap()["status"],
        "approved"
    );
    h.app.selected = 7;
    h.app.show_workflow = true;
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "Construis ton automatisation").is_some());
}

#[test]
fn clipboard_crosses_pages_and_profiles_and_undo_redo_tracks_changes() {
    let mut h = Harness::new();
    let source = h.app.config.keys()[0].clone();
    h.app.copy_key();
    h.app.config.profiles[0].pages.push(crate::model::Page::new("test-page".into(), "Test".into(), false));
    h.app.config.profiles.push(Profile::new("Second".into()));
    h.app.config.active = 1;
    h.app.config.page.clear();
    h.app.selected = 7;
    let before = h.app.config.clone();
    h.app.paste_key(7);
    h.app.record_history(before, &h.ctx);
    assert_eq!(h.app.config.keys()[7].label, source.label);
    assert_eq!(h.app.config.keys()[7].steps, source.steps);
    h.app.undo_edit();
    assert!(h.app.config.keys()[7].steps.is_empty());
    assert!(h.app.gate.dirty);
    h.app.redo_edit();
    assert_eq!(h.app.config.keys()[7].repeat, source.repeat);
    h.app.config.active = 0;
    h.app.config.page = "test-page".into();
    h.app.paste_key(4);
    assert_eq!(h.app.config.keys()[4].label, source.label);
}

#[test]
fn history_keeps_fifty_edits_and_discards_redo_after_new_change() {
    let mut h = Harness::new();
    for n in 0..55 {
        let before = h.app.config.clone();
        h.app.config.keys_mut()[0].label = format!("Edition {n}");
        h.app.record_history(before, &h.ctx);
    }
    assert_eq!(h.app.history.len(), 50);
    h.app.undo_edit();
    assert_eq!(h.app.config.keys()[0].label, "Edition 53");
    h.app.replaying = false;
    let before = h.app.config.clone();
    h.app.config.keys_mut()[0].label = "Nouvelle branche".into();
    h.app.record_history(before, &h.ctx);
    assert!(h.app.future.is_empty());
}

#[test]
fn duplicate_uses_next_empty_key_without_replacing_existing_keys() {
    let mut h = Harness::new();
    h.app.duplicate_key(0);
    assert_eq!(h.app.selected, 5);
    assert_eq!(h.app.config.keys()[5].steps, h.app.config.keys()[0].steps);
    assert_eq!(h.app.config.keys()[1].label, "Lecture");
}
#[test]
fn icon_scale_drag_shares_one_undo_entry_for_focused_field() {
    let mut h = Harness::new();
    let id = egui::Id::new("icon_scale_slider");
    h.ctx.memory_mut(|m| m.request_focus(id));
    let original = h.app.config.keys()[0].icon_scale;
    for value in [110, 130, 160] {
        let before = h.app.config.clone();
        h.app.config.keys_mut()[0].icon_scale = value;
        h.app.record_history(before, &h.ctx);
    }
    assert_eq!(h.app.history.len(), 1);
    h.app.undo_edit();
    assert_eq!(h.app.config.keys()[0].icon_scale, original);
}

#[test]
fn inspector_step_remove_buttons_stay_inside_center_panel_at_both_sizes() {
    for size in [vec2(1380.0, 900.0), vec2(1100.0, 800.0)] {
        for script_first in [true, false] {
            let mut h = Harness::new();
            let script = Step::AutoHotkey {
                executable: format!("C:\\very-long-folder\\{}\\AutoHotkey64.exe", "deep".repeat(40)),
                script: "Send \"hello\"\n".repeat(70), args: vec![],
            };
            let media = Step::Media { command: "play_pause".into() };
            h.app.config.keys_mut()[0].steps = if script_first { vec![script, media] } else { vec![media, script] };
            h.app.selected = 0;
            let output = h.warm(size);
            let buttons: Vec<_> = output.shapes.iter().filter_map(|clipped| {
                if let egui::Shape::Text(text) = &clipped.shape {
                    if text.galley.job.text == "×" && clipped.clip_rect.contains(text.pos + text.galley.size() * 0.5) {
                        return Some((text.pos.x, text.pos.x + text.galley.size().x, clipped.clip_rect.max.x));
                    }
                }
                None
            }).collect();
            assert!(!buttons.is_empty(), "remove button missing at {size:?}, script_first={script_first}");
            for (left, right, clip_right) in buttons {
                assert!(left > 240.0 && right <= clip_right && right < size.x - 296.0,
                    "remove button outside inspector: {left}..{right}, clip={clip_right}, size={size:?}");
            }
        }
    }
}

#[test]
fn workshop_entry_points_and_idle_viewport_commands() {
    let size = vec2(1380.0, 900.0);
    let mut h = Harness::new();
    h.app.config.keys_mut()[0].steps = vec![Step::AutoHotkey {
        executable: "AutoHotkey64.exe".into(), script: "Send \"test\"".into(), args: vec![],
    }];
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "Atelier de blocs").is_some());
    assert!(text_position(&output.shapes, "Ouvrir l’Atelier de blocs").is_some());
    for _ in 0..8 {
        let output = h.frame(size, vec![]);
        assert!(output.viewport_output.values().all(|v| v.commands.is_empty()), "idle frame sent a viewport command");
    }
    h.app.config.keys_mut()[0].steps.clear();
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "Partir d’un modèle").is_some());
    assert!(text_position(&output.shapes, "Ouvrir l’Atelier de blocs").is_some());
    let pos = text_position(&output.shapes, "Partir d’un modèle").unwrap();
    h.click(size, pos);
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "Modèles").is_some(), "model button opens workshop templates");

}

#[test]
fn theme_clear_color_is_opaque_and_matches_background() {
    let h = Harness::new();
    for dark in [true, false] {
        let actual = clear_background(dark);
        assert_eq!(actual[3], 1.0);
        assert_eq!(actual, if dark { [24.0/255.0,26.0/255.0,31.0/255.0,1.0] } else { [237.0/255.0,240.0/255.0,245.0/255.0,1.0] });
    }
    assert_eq!(eframe::App::clear_color(&h.app, &egui::Visuals::default()), clear_background(h.app.preferences.dark));
}
#[test]
fn mcp_connect_window_previews_all_clients_without_touching_real_settings() {
    let mut h=Harness::new();
    h.app.connect_window=true;
    let size=vec2(1280.0,850.0);
    let output=h.warm(size);
    let text=output.shapes.iter().filter_map(|shape| {
        if let egui::Shape::Text(t)=&shape.shape {Some(t.galley.job.text.as_str())}else{None}
    }).collect::<Vec<_>>().join(" ");
    for client in ["Claude Desktop","Claude Code","Codex CLI","Cursor","ChatGPT"] {assert!(text.contains(client),"Carte manquante : {client}");}
    if let Some(dir)=std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
        h.png(size,output,&PathBuf::from(dir).join("mcp-connect.png"));
    }
}
#[test]
fn plugin_cards_keep_an_import_button_visible_inside_the_library_on_small_windows() {
    for physical in [vec2(1060.0, 720.0), vec2(1366.0, 768.0)] {
        let mut h = Harness::new();
        h.app.catalog_tab = 3;
        let plugin = crate::plugins::Plugin {
            uuid: "test.demo".into(), name: "Un plugin au titre plutôt long pour le panneau".into(),
            version: "1.0.0".into(), author: "Auteur long aussi".into(), icon: None,
            folder: PathBuf::from("not-a-real-plugin"), runtime: crate::plugins::Runtime::Exe(PathBuf::from("demo.exe")),
            actions: vec![], inspector: None, node_version: None, prerequisite: None, applications: vec![],
        };
        h.app.plugin_inventory = Some(std::sync::Arc::new(super::plugin_store_ui::Inventory {
            available: vec![plugin], ..Default::default()
        }));
        let panel = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("action_catalog"));
        let mut out = h.warm(physical);
        let panel = panel.or_else(|| egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("action_catalog"))).unwrap().rect;
        let pos = panel.center();
        for _ in 0..16 {
            if text_position(&out.shapes, "Importer ce plugin").is_some() { break; }
            out = h.frame(physical, vec![Input::PointerMoved(pos), Input::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -70.0), modifiers: Modifiers::NONE }]);
        }
        let button = text_position(&out.shapes, "Importer ce plugin").expect("bouton d'importation atteignable après défilement du catalogue");
        assert!(panel.contains(button), "bouton importer doit rester dans le panneau : {button:?} / {panel:?}");
        let title = text_position(&out.shapes, "Un plugin au titre plutôt long pour le panneau")
            .expect("plugin lisible quand sa carte est visible");
        assert!(panel.contains(title), "nom hors du panneau : {title:?} / {panel:?}");
        if let Some(dir)=std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
            let dir=PathBuf::from(dir); std::fs::create_dir_all(&dir).unwrap();
            h.png(physical,out,&dir.join(format!("plugins-import-{}.png",physical.x as u32)));
        }
    }
}

#[test]
fn only_the_inspector_properties_scroll_and_its_preview_stays_fixed() {
    let size = vec2(1380.0, 900.0);
    let mut h = Harness::new();
    h.ctx.data_mut(|d| d.insert_persisted(egui::Id::new("inspector_tab"), 1u8));
    for _ in 0..12 { h.frame(size, vec![]); }
    let mut out = h.frame(size, vec![]);
    let before = text_position(&out.shapes, "Changer l'icône").unwrap();
    let header_before = text_position(&out.shapes, "Apparence").unwrap();
    for _ in 0..5 {
        out = h.frame(size, vec![Input::PointerMoved(pos2(before.x, before.y - 40.0)), Input::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -90.0), modifiers: Modifiers::NONE }]);
    }
    let after_left = text_position(&out.shapes, "Changer l'icône").expect("aperçu fixe après défilement à gauche");
    let header_after = text_position(&out.shapes, "Apparence").unwrap();
    assert!(((after_left.y - header_after.y) - (before.y - header_before.y)).abs() < 1.0, "la colonne de gauche ne doit pas défiler {before:?} {after_left:?}");
    for _ in 0..7 {
        out = h.frame(size, vec![Input::PointerMoved(pos2(before.x + 270.0, before.y - 40.0)), Input::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -90.0), modifiers: Modifiers::NONE }]);
    }
    assert!(text_position(&out.shapes, "Taille de l’icône").is_some(), "la colonne de droite doit défiler");
    let after_right = text_position(&out.shapes, "Changer l'icône").expect("aperçu fixe après défilement à droite");
    let header_final = text_position(&out.shapes, "Apparence").unwrap();
    assert!(((after_right.y - header_final.y) - (before.y - header_before.y)).abs() < 1.0, "aperçu ne suit pas le défilement des propriétés");
    if let Some(dir)=std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
        let dir=PathBuf::from(dir); std::fs::create_dir_all(&dir).unwrap();
        h.png(size,out,&dir.join("inspector-right-scroll.png"));
    }
}

#[test]
fn plugin_store_is_rendered_without_a_window() {
    let mut h=Harness::new();
    h.app.catalog_tab=3;
    // 0.9.0 : le bouton « Magasin de plugins » précède l'inventaire ; l'onglet défile en dessous.
    let size=vec2(1180.0,860.0);
    let output=h.warm(size);
    assert!(text_position(&output.shapes,"Installés dans NovyDeck").is_some());
    assert!(text_position(&output.shapes,"Plugins disponibles sur ce PC").is_some());
    if let Some(dir)=std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
        h.png(size,output,&PathBuf::from(dir).join("plugins-store.png"));
    }
    // Régression : l'onglet scannait les dossiers de plugins (et lançait reg.exe) à chaque image.
    let first=h.app.plugin_inventory.clone().expect("inventaire mis en cache");
    for _ in 0..5 { h.warm(size); }
    assert!(std::sync::Arc::ptr_eq(&first,h.app.plugin_inventory.as_ref().unwrap()),"l'onglet Plugins ne doit pas relire le disque à chaque image");
}

#[test]
fn settings_tabs_fit_a_laptop_screen_and_ai_opens_the_single_assistant_window() {
    let size = vec2(1366.0, 768.0);
    let mut h = Harness::new();
    h.app.settings = true;
    for tab in 0..3u8 {
        h.app.settings_tab = tab;
        h.warm(size);
        let rect = h.ctx.memory(|m| m.area_rect(egui::Id::new("device_settings"))).expect("Réglages affichés");
        assert!(rect.max.y <= size.y && rect.min.y >= 0.0, "onglet {tab} : la fenêtre dépasse l'écran {rect:?}");
    }
    h.app.settings_tab = 0;
    let general = h.warm(size);
    assert!(text_position(&general.shapes, "IA : désactivée").is_some() && !h.app.preferences.local_ai.enabled, "IA locale désactivée par défaut");
    let button = text_position(&general.shapes, "Configurer…").expect("entrée Assistant IA");
    h.click(size, button);
    assert!(h.app.assistant_settings && h.app.assistant_tab == 0, "ouvre l'Assistant IA sur l'onglet IA locale");
    h.app.settings = false;
    h.app.preferences.local_ai.enabled = true;
    h.app.preferences.local_ai.backend = "ollama".into();
    let open = h.warm(size);
    for text in ["IA locale (sur ce PC)", "IA connectée (Claude, ChatGPT…)", "Utiliser une IA locale", "Ollama", "Intégré à NovyDeck", "Tester", "IA : locale"] {
        assert!(text_position(&open.shapes, text).is_some(), "{text}");
    }
    assert!(text_position(&open.shapes, "Connecter une IA").is_none(), "plus de bouton IA en double dans l'en-tête");
    assert!(text_position(&open.shapes, "Arrêter les macros en cours").is_none() && text_position(&open.shapes, "Arrêter les macros").is_none(), "Arrêter n'apparaît que pendant une exécution");
    if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
        std::fs::create_dir_all(&dir).unwrap();
        h.png(size, open, &PathBuf::from(&dir).join("assistant-ia-locale-ollama.png"));
        h.app.preferences.local_ai.backend = "integre".into();
        let out = h.warm(size);
        h.png(size, out, &PathBuf::from(&dir).join("assistant-ia-locale-integre.png"));
        h.app.assistant_tab = 1;
        let out = h.warm(size);
        h.png(size, out, &PathBuf::from(&dir).join("assistant-ia-connectee.png"));
        h.app.assistant_settings = false;
        h.app.settings = true;
        for (tab, name) in [(0u8, "reglages-general"), (1, "reglages-peripherique"), (2, "reglages-diagnostic")] {
            h.app.settings_tab = tab;
            let out = h.warm(size);
            h.png(size, out, &PathBuf::from(&dir).join(format!("{name}.png")));
        }
    }
}

#[test]
fn rules_preview_offers_the_local_ai_when_it_is_enabled() {
    let size = vec2(1380.0, 900.0);
    let mut h = Harness::new();
    h.app.show_workflow = true;
    h.app.workshop_chat.open = true;
    h.app.workshop_chat_send("colle sans mise en forme".into(), None);
    let out = h.warm(size);
    assert!(text_position(&out.shapes, "Pas ça ? Demander plutôt à l'IA locale").is_none(), "IA locale désactivée : pas de bouton");
    h.app.preferences.local_ai.enabled = true;
    h.app.preferences.local_ai.endpoint = "http://127.0.0.1:9".into();
    h.app.workshop_chat_send("colle sans mise en forme".into(), None);
    assert!(h.app.workshop_chat.pending.as_ref().is_some_and(|p| !p.ai && p.proposal.steps.len() == 3), "proposition des règles d'abord");
    let out = h.warm(size);
    let button = text_position(&out.shapes, "Pas ça ? Demander plutôt à l'IA locale").expect("bouton IA locale");
    h.click(size, button);
    assert!(h.app.workshop_chat.thinking.is_some() && h.app.workshop_chat.pending.is_none(), "la demande part vers l'IA locale, sans insertion");
}

#[test]
fn library_scroll_reuses_cached_interface_icons() {
    let size = vec2(1380.0, 900.0);
    let mut h = Harness::new();
    let first = h.warm(size);
    let shown = crate::iconset::rasterized(&h.ctx);
    assert!(shown >= 8, "icônes IconPark affichées : {shown}");
    assert!(text_position(&first.shapes, "Réglages").is_some() && text_position(&first.shapes, "Magasin").is_some());
    let list = egui::pos2(1230.0, 520.0);
    let wheel = |dy: f32| vec![Input::PointerMoved(list), Input::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, dy), modifiers: Modifiers::NONE }];
    for tab in [0usize, 1] {
        h.app.catalog_tab = tab;
        for dy in [-120.0f32; 25].into_iter().chain([120.0f32; 25]) {
            h.frame(size, wheel(dy));
        }
    }
    let after_first_pass = crate::iconset::rasterized(&h.ctx);
    let started = std::time::Instant::now();
    for tab in [0usize, 1] {
        h.app.catalog_tab = tab;
        for dy in [-120.0f32; 25].into_iter().chain([120.0f32; 25]) {
            h.frame(size, wheel(dy));
        }
    }
    let per_frame = started.elapsed() / 100;
    assert_eq!(crate::iconset::rasterized(&h.ctx), after_first_pass, "aucune icône n'est rastérisée de nouveau en défilant");
    eprintln!("ICONSET {after_first_pass} textures, {per_frame:?} par image (debug)");
}

#[test]
fn startup_intro_page_fade_and_waiting_dot_animate_without_blocking() {
    let size = vec2(1380.0, 900.0);
    let mut h = Harness::new();
    h.app.preferences.reduce_motion = false;
    let first = h.frame(size, vec![]);
    // Le nom de l'intro est le seul « NovyDeck » en 26 pt (l'en-tête est en 18 pt).
    let intro = |o: &egui::FullOutput| o.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t)
        if t.galley.job.text == "NovyDeck" && t.galley.job.sections.first().is_some_and(|f| f.format.font_id.size == 26.0)));
    assert!(intro(&first), "intro visible au démarrage");
    if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
        std::fs::create_dir_all(&dir).unwrap();
        for _ in 0..8 { h.frame(size, vec![]); }
        let out = h.frame(size, vec![]);
        h.png(size, out, &PathBuf::from(&dir).join("intro-1.png"));
        for _ in 0..4 { h.frame(size, vec![]); }
        let out = h.frame(size, vec![]);
        h.png(size, out, &PathBuf::from(&dir).join("intro-2.png"));
    }
    // L'intro ne capte pas la souris : un clic sur une touche passe à travers.
    let key = h.app.drop_targets[3].0.center();
    h.click(size, key);
    assert_eq!(h.app.selected, 3, "l'intro ne bloque pas les clics");
    for _ in 0..30 { h.frame(size, vec![]); }
    let later = h.frame(size, vec![]);
    assert!(!intro(&later), "l'intro disparaît d'elle-même");
    // Réduire les animations : plus d'intro.
    let mut calm = Harness::new();
    let out = calm.frame(size, vec![]);
    assert!(!intro(&out));
}

#[test]
fn factory_example_keys_keep_actions_and_icons_but_use_no_colored_lcd_background() {
    let cfg = Config::default();
    for (i, label) in [(0, "Exemple macro"), (1, "Lecture")] {
        let key = &cfg.keys()[i];
        assert_eq!(key.label, label);
        assert!(!key.steps.is_empty(), "actions conservées");
        assert!(key.image.is_empty(), "icônes par défaut conservées");
        assert!(key.background, "LCD cohérent avec les autres touches");
        assert_eq!(key.color, Key::default().color, "aucun fond violet ou vert sur {label}");
    }
}

#[test]
fn svg_chassis_aligns_the_fifteen_live_keys_and_renders_once() {
    let dir = tempfile::tempdir().unwrap();
    // Contenus variés : image PNG, GIF animé, icône par défaut, titre seul, touche vide.
    let png = dir.path().join("photo.png");
    image::RgbImage::from_fn(96, 96, |x, y| image::Rgb([(x * 2) as u8, (y * 2) as u8, 180])).save(&png).unwrap();
    let gif = dir.path().join("anim.gif");
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(std::fs::File::create(&gif).unwrap());
        encoder.set_repeat(image::codecs::gif::Repeat::Infinite).unwrap();
        for c in [40u8, 200] {
            let frame = image::RgbaImage::from_pixel(64, 64, image::Rgba([c, 90, 255 - c, 255]));
            encoder.encode_frame(image::Frame::from_parts(frame, 0, 0, image::Delay::from_numer_denom_ms(200, 1))).unwrap();
        }
    }
    for (size, scale, name) in [(vec2(1380.0, 900.0), 1.0, "deck-1380"), (vec2(1060.0, 720.0), 1.0, "deck-1060"), (vec2(1920.0, 1080.0) / 1.25, 1.25, "deck-1920-125")] {
        let mut h = Harness::new();
        h.ctx.set_pixels_per_point(scale);
        let keys = h.app.config.keys_mut();
        keys[5].image = png.to_string_lossy().into_owned();
        keys[5].label = "Photo".into();
        keys[6].image = gif.to_string_lossy().into_owned();
        keys[7] = Key { label: "Stop".into(), steps: vec![Step::Media { command: "stop".into() }], ..Default::default() };
        keys[8] = Key { label: "Page +".into(), steps: vec![Step::PageNext], ..Default::default() };
        keys[9] = Key { steps: vec![Step::Hotkey { keys: "CTRL+C".into() }], ..Default::default() };
        keys[10] = Key { label: "Titre seul".into(), ..Default::default() };
        let out = h.warm(size);
        let rects: Vec<Rect> = (0..15).map(|i| h.app.drop_targets.iter().find(|(_, k)| *k == i).expect("touche présente").0).collect();
        let w = rects[0].width();
        for (i, r) in rects.iter().enumerate() {
            assert!(r.min.x >= 0.0 && r.max.x <= size.x && r.min.y >= 0.0 && r.max.y <= size.y, "{name} : touche {i} visible {r:?}");
            assert!((r.width() / r.height() - crate::deck_chassis::FACE_SIZE.x / crate::deck_chassis::FACE_SIZE.y).abs() < 0.01, "{name} : face aux proportions du SVG");
            let (col, row) = ((i % 5) as f32, (i / 5) as f32);
            let expected = rects[0].min + vec2(248.0 * col, 232.0 * row) * (w / crate::deck_chassis::FACE_SIZE.x);
            assert!((r.min - expected).length() < 0.6, "{name} : touche {i} alignée sur son emplacement ({r:?} / {expected:?})");
        }
        assert!(w >= 40.0, "{name} : touches assez grandes ({w})");
        let loaded = crate::deck_chassis::textures_loaded(&h.ctx);
        for _ in 0..20 { h.frame(size, vec![]); }
        assert!(loaded <= 2 && crate::deck_chassis::textures_loaded(&h.ctx) == loaded, "{name} : châssis jamais rendu de nouveau image après image");
        // Clic et sélection inchangés.
        h.click(size, rects[12].center());
        assert_eq!(h.app.selected, 12, "{name} : clic sur la touche 13");
        if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT").or_else(|| std::env::var_os("DECKFORGE_UI_OUTPUT")) {
            std::fs::create_dir_all(&dir).unwrap();
            let out = h.warm(size);
            h.png(size, out, &PathBuf::from(&dir).join(format!("{name}.png")));
        }
        let _ = out;
    }
}

#[test]
fn flicker_ui_start_mouse_transition_stop_writes_real_trace() {
    let mut h = Harness::new();
    let size = vec2(1380.0, 900.0);
    h.app.settings = true;
    h.app.settings_tab = 2;
    let open = h.warm(size);
    let button = text_position(&open.shapes, "Démarrer le diagnostic (90 s max)").expect("diagnostic button visible in settings");
    h.click(size, button);
    let path = h.app.flicker_path.clone().expect("button starts trace");
    h.app.settings = false;
    h.frame(size, vec![]);
    let preview = h.app.diagnostic_deck.expect("preview measured").center();
    let inspector = h.app.diagnostic_inspector.expect("inspector measured").center();
    h.frame(size, vec![Input::PointerMoved(preview)]);
    h.frame(size, vec![Input::PointerMoved(inspector)]);
    h.app.flicker.as_ref().unwrap().stop();
    let deadline=Instant::now()+Duration::from_secs(4);
    let text=loop {
        let text=std::fs::read_to_string(&path).unwrap();
        if text.lines().any(|s|s.contains("\"kind\":\"stop\"")) {break text;}
        assert!(Instant::now()<deadline,"trace not flushed");
        std::thread::sleep(Duration::from_millis(20));
    };
    let entries:Vec<serde_json::Value>=text.lines().map(|s|serde_json::from_str(s).unwrap()).collect();
    assert!(entries.iter().any(|v|v["zone"]=="preview"));
    assert!(entries.iter().any(|v|v["zone"]=="inspector"));
    assert!(entries.iter().any(|v|v["metrics"]["preview_rect"].is_array() && v["metrics"]["inspector_rect"].is_array()));
    assert!(text.len()<6*1024*1024);
    if let Some(out)=std::env::var_os("DECKFORGE_FLICKER_TEST_OUTPUT") {std::fs::copy(path,out).unwrap();}
}

#[test]
fn visible_chassis_brand_is_novydeck() {
    let size = vec2(1380.0, 900.0);
    let mut h = Harness::new();
    let out = h.warm(size);
    assert!(text_position(&out.shapes, "NOVYDECK").is_some(), "marque NOVYDECK visible sur le châssis");
    assert!(text_position(&out.shapes, "DECKFORGE").is_none(), "ancienne marque DECKFORGE absente du châssis");
}

#[test]
fn about_dialog_shows_cargo_version_without_network() {
    let size = vec2(1380.0, 900.0);
    let mut h = Harness::new();
    h.app.show_about = true;
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "À propos de NovyDeck").is_some());
    assert!(text_position(&output.shapes, &format!("Version : {}", env!("CARGO_PKG_VERSION"))).is_some());
    assert!(text_position(&output.shapes, "Vérifier les mises à jour").is_some());
    assert!(h.app.update_rx.is_none());
}

#[test]
fn inspector_dpi_keeps_keys_and_primary_actions_visible_without_button_overlap() {
    for physical in [vec2(1920.0, 1080.0), vec2(1366.0, 768.0), vec2(1060.0, 720.0)] {
        for scale in [1.0_f32, 1.25, 1.5] {
            if (physical.x < 1300.0) != (scale == 1.0) { continue; }
            let size = physical / scale;
            let mut h = Harness::new();
            h.ctx.set_pixels_per_point(scale);
            let output = h.warm(size);
            assert_eq!(h.app.drop_targets.len(), 15, "{physical:?} @ {scale}");
            eprintln!("physical={physical:?} scale={scale} logical={size:?} screen={:?} last={:?} insp={:?} first={:?}", h.ctx.screen_rect(), h.app.drop_targets[14].0, egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("key_inspector")).map(|s| s.rect), h.app.drop_targets[0].0);
            for (r, index) in &h.app.drop_targets {
                assert!(r.min.x >= 0.0 && r.min.y >= 0.0 && r.max.x <= size.x && r.max.y <= size.y,
                    "Key {} clipped at {physical:?} @ {scale}: {r:?}", index + 1);
                // Faces aux proportions du châssis SVG (202 × 190) : 40 × 38 points minimum (50 × 48 px à 125 %).
                assert!(r.width() >= 40.0 && r.height() >= 37.5, "{physical:?} @ {scale}: key {index} {r:?}");
            }
            for name in ["Simuler", "Tester pour de vrai", "Atelier de blocs"] {
                assert!(text_position(&output.shapes, name).is_some(), "{name} hidden at {physical:?} @ {scale}");
            }
            let buttons: Vec<Rect> = h.ctx.data(|d| d.get_temp(egui::Id::new("inspector_button_rects"))).unwrap();
            assert_eq!(buttons.len(), 3);
            if let Some(toggle) = h.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("inspector_press_toggle"))) {
                for b in &buttons {
                    assert!(!toggle.intersects(*b), "« Appui court » recouvert à {physical:?} @ {scale}: {toggle:?} {b:?}");
                }
            }
            for (i, a) in buttons.iter().enumerate() {
                assert!(a.min.x >= 0.0 && a.max.x <= size.x && a.min.y >= 0.0 && a.max.y <= size.y, "button clipped: {a:?}");
                for b in buttons.iter().skip(i + 1) {
                    assert!(!a.intersects(*b), "inspector buttons overlap at {physical:?} @ {scale}: {a:?} {b:?}");
                }
            }
        }
    }
}

#[test]
fn inspector_recrop_button_stays_visible_under_the_key_preview() {
    // Retour utilisateur : « Recadrer » était coupé par le bas du panneau sous « Changer l'icône ».
    fn visible(shapes: &[egui::epaint::ClippedShape], label: &str) -> Option<(egui::Rect, egui::Rect)> {
        fn find(shape: &egui::Shape, label: &str) -> Option<egui::Rect> {
            match shape {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    assert_eq!(t.galley.rows.len(), 1, "{label} ne doit pas passer sur deux lignes");
                    // Le cadre du bouton dépasse le texte du rembourrage vertical (4 px).
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()).expand2(vec2(0.0, 4.0)))
                }
                egui::Shape::Vec(v) => v.iter().find_map(|s| find(s, label)),
                _ => None,
            }
        }
        shapes.iter().find_map(|s| find(&s.shape, label).map(|r| (r, s.clip_rect)))
    }
    let mut checked = 0;
    for height in [700.0, 760.0, 820.0, 900.0, 1000.0, 1100.0] {
        let size = vec2(1600.0, height);
        let mut h = demo_harness();
        h.app.selected = h.app.config.keys().iter().position(|k| k.label == "Photo").expect("touche Photo du profil de démonstration");
        h.ctx.data_mut(|d| d.insert_persisted(egui::Id::new("inspector_tab"), 1u8));
        let out = settle(&mut h, size);
        // Panneau trop bas : l'aperçu seul est affiché, sans boutons (comportement voulu).
        if visible(&out.shapes, "Changer l'icône").is_none() { continue; }
        checked += 1;
        for label in ["Changer l'icône", "Recadrer"] {
            // egui ne peint pas un bouton hors de la zone visible : absent = coupé.
            let (rect, clip) = visible(&out.shapes, label).unwrap_or_else(|| panic!("{label} invisible (coupé) à la hauteur {height}"));
            assert!(clip.contains_rect(rect), "{label} coupé à la hauteur {height} : {rect:?} hors de {clip:?}");
        }
    }
    assert!(checked >= 2, "les boutons de l'aperçu doivent apparaître pour au moins deux hauteurs de fenêtre");
}

#[test]
fn community_library_waits_for_consent_below_local_plugins() {
    let mut h = Harness::new();
    h.app.catalog_tab = 3;
    let out = h.warm(vec2(1180.0, 800.0));
    let installed = text_position(&out.shapes, "Installés dans NovyDeck").expect("inventaire local visible");
    let store = text_position(&out.shapes, "Magasin de plugins").expect("bouton magasin présent");
    assert!(store.y < installed.y, "le magasin doit précéder l'inventaire local");
    assert!(text_position(&out.shapes, "Se connecter au magasin").is_none(), "aucune connexion avant ouverture du magasin");
}

/// Contrôle manuel (réseau + captures) : `cargo test -- --ignored release_0812_review`.
#[test]
#[ignore]
fn release_0812_review() {
    // Vrai catalogue GitHub via WinHTTP (HTTPS, redirections).
    let entries = crate::community_plugins::fetch().expect("catalogue communautaire joignable");
    println!("COMMUNITY_ENTRIES={}", entries.len());
    let dir = PathBuf::from(std::env::var_os("NOVYDECK_UI_OUTPUT").expect("NOVYDECK_UI_OUTPUT"));
    std::fs::create_dir_all(&dir).unwrap();
    let size = vec2(1380.0, 900.0);
    let mut h = demo_harness();
    h.app.open_hardware_report();
    let report = h.app.hardware_report.as_ref().unwrap();
    let user = std::env::var("USERNAME").unwrap_or_default();
    assert!(user.len() < 3 || !report.text.to_lowercase().contains(&user.to_lowercase()), "nom d'utilisateur présent dans le rapport");
    std::fs::write(dir.join("report-preview.txt"), &report.text).unwrap();
    std::fs::write(dir.join("report-issue-url.txt"), &report.issue_url).unwrap();
    let out = settle(&mut h, size);
    h.png(size, out, &dir.join("report-window.png"));
    let mut h = demo_harness();
    h.app.selected = h.app.config.keys().iter().position(|k| k.label == "Photo").unwrap();
    h.ctx.data_mut(|d| d.insert_persisted(egui::Id::new("inspector_tab"), 1u8));
    let out = settle(&mut h, vec2(1600.0, 820.0));
    h.png(vec2(1600.0, 820.0), out, &dir.join("inspector-recrop-820.png"));
    let mut h = Harness::new();
    h.app.catalog_tab = 3;
    let out = h.warm(vec2(1180.0, 2400.0));
    h.png(vec2(1180.0, 2400.0), out, &dir.join("plugins-community.png"));
}

#[test]
fn update_offer_widgets_follow_installation_mode_without_network() {
    let size = vec2(1380.0, 900.0);
    let latest = crate::app_update::Latest {
        version: "v0.8.14".into(),
        page: "https://github.com/HatiFlash/NovyDeck/releases/tag/v0.8.14".into(),
        setup_url: "https://github.com/x/setup.exe".into(),
        setup_size: 100,
        sha_url: "https://github.com/x/setup.exe.sha256".into(),
        notes: "## Français\n- Une nouveauté testée".into(),
    };
    for installed in [true, false] {
        let mut h = Harness::new();
        h.app.show_about = true;
        h.app.update_installed = installed;
        h.app.update_result = Some(Ok(crate::app_update::Status::New(latest.clone())));
        let output = h.warm(size);
        assert!(text_position(&output.shapes, "Nouvelle version v0.8.14 disponible").is_some());
        assert!(text_position(&output.shapes, "Voir les nouveautés").is_some());
        assert_eq!(text_position(&output.shapes, "Télécharger et installer").is_some(), installed);
        assert_eq!(text_position(&output.shapes, "Version portable : téléchargez le ZIP depuis la page des nouveautés").is_some(), !installed);
        assert!(h.app.update_rx.is_none());
    }
}

#[test]
fn audio_switch_can_be_dropped_before_choosing_its_devices() {
    // Retour utilisateur : « Affectation refusée : Choisis deux périphériques audio différents »
    // en glissant l'action sur une touche, alors que les périphériques se règlent ensuite.
    let mut h = Harness::new();
    h.app.assign(DragItem::Action(catalog::action(13, "Bureau")), 6);
    assert!(matches!(&h.app.config.keys()[6].steps[..], [Step::AudioSwitch { first, second, .. }] if first.is_empty() && second.is_empty()),
        "la touche doit recevoir l'action audio à configurer : {}", h.app.notice);
    assert!(h.app.notice.contains("Choisis maintenant les deux périphériques audio"), "{}", h.app.notice);
    // Les deux mêmes périphériques restent refusés une fois choisis.
    let mut same = catalog::action(13, "Bureau");
    if let Step::AudioSwitch { first, second, .. } = &mut same.steps[0] { *first = "casque".into(); *second = "casque".into(); }
    let before = h.app.config.keys()[7].clone();
    h.app.assign(DragItem::Action(same), 7);
    assert_eq!(h.app.config.keys()[7].steps, before.steps);
    assert!(h.app.notice.contains("deux périphériques audio différents"), "{}", h.app.notice);
}

#[test]
fn english_main_window_uses_translated_labels() {
    let mut h = Harness::new();
    crate::i18n::set_language("en");
    let output = h.warm(vec2(1380.0, 900.0));
    for (english, french) in [
        ("Action library", "Bibliothèque d'actions"),
        ("Search for an action…", "Rechercher une action…"),
        ("Settings", "Réglages"),
    ] {
        assert!(text_position(&output.shapes, english).is_some(), "missing {english}");
        assert!(text_position(&output.shapes, french).is_none(), "still French: {french}");
    }
}

#[test]
fn language_combo_is_visible_in_general_settings() {
    let mut h = Harness::new();
    h.app.settings = true;
    h.app.settings_tab = 0;
    let output = h.warm(vec2(1380.0, 900.0));
    assert!(text_position(&output.shapes, "Langue").is_some());
    assert!(text_position(&output.shapes, "Langue de Windows").is_some());
    assert!(text_position(&output.shapes, "Aider à traduire NovyDeck").is_some());
}

#[test]
fn startup_consent_requires_an_unset_preference() {
    let size = vec2(1380.0, 900.0);
    let mut h = Harness::new();
    h.app.startup_prompts = true;
    h.app.startup_checked = true; // Never contact GitHub in UI tests.
    let shown = h.warm(size);
    assert!(text_position(&shown.shapes, "Mises à jour automatiques").is_some());
    assert!(text_position(&shown.shapes, "Vérifier au démarrage").is_some());
    let reject = text_position(&shown.shapes, "Non merci").unwrap();
    h.click(size, reject);
    assert_eq!(h.app.preferences.update_check, Some(false));
    assert!(text_position(&h.warm(size).shapes, "Mises à jour automatiques").is_none());
    h.app.preferences.update_check = Some(true);
    assert!(text_position(&h.warm(size).shapes, "Mises à jour automatiques").is_none());
    assert!(h.app.startup_rx.is_none());
}

#[test]
fn whats_new_is_only_for_existing_installations_with_new_notes() {
    let size = vec2(1380.0, 900.0);
    let mut h = Harness::new();
    let notes = "## Français\n- Changement après mise à jour";
    assert!(!App::needs_whats_new(&h.app.preferences, false, notes));
    assert!(App::needs_whats_new(&h.app.preferences, true, notes));
    h.app.preferences.last_seen_version = "0.8.13".into();
    h.app.preferences.update_check = Some(false);
    h.app.startup_prompts = true;
    h.app.whats_new = true;
    h.app.whats_new_notes = notes.into();
    let shown = h.warm(size);
    let title = format!("Quoi de neuf dans NovyDeck {}", env!("CARGO_PKG_VERSION"));
    assert!(text_position(&shown.shapes, &title).is_some());
    assert!(text_position(&shown.shapes, "- Changement après mise à jour").is_some());
    let acknowledge = text_position(&shown.shapes, "Compris").unwrap();
    h.click(size, acknowledge);
    assert_eq!(h.app.preferences.last_seen_version, env!("CARGO_PKG_VERSION"));
    assert!(!h.app.whats_new);
    assert!(text_position(&h.warm(size).shapes, &title).is_none());
}

#[test]
fn plugin_action_two_virtual_contexts_execute_and_persist() {
    let mut h = Harness::new();
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/counter.sdPlugin");
    let plugin = crate::plugins::parse(&folder, &std::fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    let action = plugin.actions[0].uuid.clone();
    h.app.config.keys_mut()[0].steps = vec![
        Step::PluginAction { plugin: plugin.uuid.clone(), action: action.clone(), settings: serde_json::json!({}), label: "Compteur A".into() },
        Step::Delay { ms: 100 },
        Step::PluginAction { plugin: plugin.uuid.clone(), action: action.clone(), settings: serde_json::json!({"count":1}), label: "Compteur B".into() },
    ];
    h.app.applied = h.app.config.clone();
    h.app.plugins = Some(crate::plugin_runtime::Manager::new(vec![plugin]));
    h.app.plugins.as_mut().unwrap().sync(&h.app.applied);
    let identity = crate::engine::identity(&h.app.config.profiles[h.app.config.active].name, &h.app.config.page, 0);
    assert_eq!(crate::plugin_runtime::virtual_step_path(&h.app.applied, &crate::plugin_runtime::virtual_context(&identity, &[0])), Some((0, vec![0])));
    h.app.start_workshop_run(false);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        h.app.events();
        h.app.plugin_events();
        let steps = &h.app.config.keys()[0].steps;
        let counts = [0, 2].map(|i| match &steps[i] { Step::PluginAction { settings, .. } => settings["count"].as_i64(), _ => None });
        if counts == [Some(1), Some(2)] { break; }
        assert!(Instant::now() < deadline, "PluginAction settings timeout: {counts:?}, errors: {:?}, journal: {:?}, error: {:?}, running: {}", h.app.plugin_errors, h.app.workshop_journal.lines.iter().map(|l| &l.message).collect::<Vec<_>>(), h.app.workshop_journal.error, h.app.engine.running.load(std::sync::atomic::Ordering::SeqCst));
        std::thread::sleep(Duration::from_millis(10));
    }
    let saved = crate::store::load(&h.dir.path().join("config.json")).unwrap();
    for (i, count) in [(0, 1), (2, 2)] {
        assert!(matches!(&saved.keys()[0].steps[i], Step::PluginAction { settings, .. } if settings["count"] == count));
    }
    let id = crate::engine::identity(&h.app.config.profiles[h.app.config.active].name, &h.app.config.page, 0);
    assert_ne!(crate::plugin_runtime::virtual_context(&id, &[0]), crate::plugin_runtime::virtual_context(&id, &[2]));
    assert!(h.app.plugin_errors.is_empty(), "{:?}", h.app.plugin_errors);
}

#[test]
fn startup_offer_shows_notes_and_installation_choice_without_network() {
    let size = vec2(1380.0, 900.0);
    let mut h = Harness::new();
    h.app.startup_checked = true;
    h.app.preferences.update_check = Some(true);
    h.app.startup_prompts = true;
    h.app.update_installed = true;
    h.app.startup_offer = Some(crate::app_update::Latest {
        version: "v0.8.16".into(), page: "https://github.com/HatiFlash/NovyDeck".into(),
        setup_url: "https://github.com/x/setup.exe".into(), setup_size: 100,
        sha_url: "https://github.com/x/setup.sha256".into(),
        notes: "## Français\n- Fonction nouvelle injectée".into(),
    });
    let shown = h.warm(size);
    for label in ["Mise à jour disponible", "- Fonction nouvelle injectée", "Télécharger et installer", "Voir les nouveautés", "Plus tard"] {
        assert!(text_position(&shown.shapes, label).is_some(), "{label}");
    }
    assert!(h.app.startup_rx.is_none());
}

#[test]
fn market_opens_offline_and_displays_localized_detail() {
    use super::plugin_store_ui::community_state;
    let mut h=Harness::new();
    h.app.catalog_tab=3;
    let size=vec2(1366.0,768.0);
    let mut fixture=crate::community_plugins::parse(&serde_json::to_vec(&serde_json::json!({"schema":1,"plugins":[{"id":"com.example.test","uuid":"com.example.test","name":"Plugin témoin","author":"Alice","version":"1.0","description":"English details","description_fr":"Description française","summary":"English summary","summary_fr":"Résumé français","category":"audio","homepage":"https://example.org/","download":"https://example.org/test.zip","sha256":"a".repeat(64),"license":"MIT","tested_decks":[],"min_novydeck":"0.8.12","icon":"https://example.org/icon.png","screenshots":["https://example.org/shot.png"]}]})).unwrap()).unwrap();
    fixture[0].compatible=true;
    let first=h.warm(size);
    let store=text_position(&first.shapes,"Magasin de plugins").unwrap();
    let installed=text_position(&first.shapes,"Installés dans NovyDeck").unwrap();
    assert!(store.y<installed.y);
    h.click(size,store);
    let waiting=h.warm(size);
    assert!(text_position(&waiting.shapes,"Se connecter au magasin").is_some());
    assert!(text_position(&waiting.shapes,"Plugin témoin").is_none());
    {
        let mut state=community_state().lock().unwrap();
        state.consent=true;
        state.entries=fixture;
    }
    super::plugin_market_ui::test_image(&h.ctx,"https://example.org/icon.png");
    super::plugin_market_ui::test_image(&h.ctx,"https://example.org/shot.png");
    let grid=h.warm(size);
    if let Some(dir)=std::env::var_os("NOVYDECK_UI_OUTPUT") { h.png(size,grid.clone(),&PathBuf::from(dir).join("market-grid.png")); }
    assert!(text_position(&grid.shapes,"Plugin témoin").is_some());
    let details=text_position(&grid.shapes,"Voir la fiche").unwrap();
    h.click(size,details);
    let detail=h.warm(size);
    if let Some(dir)=std::env::var_os("NOVYDECK_UI_OUTPUT") { h.png(size,detail.clone(),&PathBuf::from(dir).join("market-detail.png")); }
    for text in ["Description française","Installer","Captures"] {assert!(text_position(&detail.shapes,text).is_some(),"{text}");}
    crate::i18n::set_language("en");
    let english=h.warm(size);
    assert!(text_position(&english.shapes,"English details").is_some());
    crate::i18n::set_language("fr");
    {
        let mut state=community_state().lock().unwrap();
        state.consent=false;state.entries.clear();
    }
}

#[test]
fn plugin_action_two_installed_plugins_run_in_sequence() {
    let mut h = Harness::new();
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/counter.sdPlugin");
    let first = crate::plugins::parse(&folder, &std::fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    let mut second = first.clone();
    second.uuid = "org.deckforge.counter.second".into();
    let action = first.actions[0].uuid.clone();
    h.app.config.keys_mut()[0].steps = vec![
        Step::PluginAction { plugin: first.uuid.clone(), action: action.clone(), settings: serde_json::json!({}), label: "Premier".into() },
        Step::Delay { ms: 150 },
        Step::PluginAction { plugin: second.uuid.clone(), action, settings: serde_json::json!({}), label: "Second".into() },
    ];
    h.app.applied = h.app.config.clone();
    h.app.plugins = Some(crate::plugin_runtime::Manager::new(vec![first, second]));
    h.app.plugins.as_mut().unwrap().sync(&h.app.applied);
    let identity = crate::engine::identity(&h.app.config.profiles[h.app.config.active].name, &h.app.config.page, 0);
    assert_eq!(crate::plugin_runtime::virtual_step_path(&h.app.applied, &crate::plugin_runtime::virtual_context(&identity, &[0])), Some((0, vec![0])));
    h.app.start_workshop_run(false);
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut first_seen = false;
    loop {
        h.app.events(); h.app.plugin_events();
        let counts = [0, 2].map(|i| match &h.app.config.keys()[0].steps[i] { Step::PluginAction { settings, .. } => settings["count"].as_i64(), _ => None });
        if counts[0] == Some(1) { first_seen = true; }
        if counts[1] == Some(1) { assert!(first_seen, "second plugin responded before first"); break; }
        assert!(Instant::now() < deadline, "Two-plugin timeout: {counts:?}, errors: {:?}", h.app.plugin_errors);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(h.app.plugin_errors.is_empty(), "{:?}", h.app.plugin_errors);
}

#[test]
fn workshop_plugin_catalogue_adds_action_card_and_capture() {
    let mut h = Harness::new();
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/counter.sdPlugin");
    let mut plugin = crate::plugins::parse(&folder, &std::fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    plugin.name = "Atelier Témoin".into();
    plugin.actions[0].name = "Incrémenter".into();
    let expected = format!("{} › {}", plugin.display_name(), plugin.actions[0].name);
    h.app.plugins = Some(crate::plugin_runtime::Manager::new(vec![plugin.clone()]));
    h.app.show_workflow = true;
    h.app.config.keys_mut()[h.app.selected].steps.clear();
    let size = vec2(1600.0, 1000.0);
    let output = h.warm(size);
    let category = text_position(&output.shapes, "Plugins").expect("plugin category");
    h.click(size, category);
    let output = h.warm(size);
    let action_pos = text_position(&output.shapes, &expected).expect("installed action in palette");
    h.click(size, action_pos);
    let output = h.warm(size);
    assert!(matches!(&h.app.config.keys()[h.app.selected].steps[..], [Step::PluginAction { plugin: id, action, .. }] if id == &plugin.uuid && action == &plugin.actions[0].uuid));
    assert!(text_position(&output.shapes, &expected).is_some(), "plugin/action card label");
    if let Some(dir) = std::env::var_os("NOVYDECK_UI_OUTPUT") {
        h.png(size, output, &PathBuf::from(dir).join("workshop-plugins.png"));
    }
}
