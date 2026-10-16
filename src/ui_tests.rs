//! Tests of the actual egui widgets, without opening or controlling Windows.
use super::*;
use egui::{pos2, vec2, Event as Input, Modifiers, PointerButton, Rect};

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
        let app = App::init(&ctx, Some(dir.path().join("config.json")));
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
            events,
            focused: true,
            ..Default::default()
        };
        let output = self.ctx.run(input, |ctx| self.app.frame(ctx));
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
        let width = size.x as u32;
        let height = size.y as u32;
        let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([24, 26, 31, 255]));
        let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        for primitive in primitives {
            let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
                continue;
            };
            let Some(texture) = self.textures.get(&mesh.texture_id) else {
                continue;
            };
            for triangle in mesh.indices.chunks_exact(3) {
                let a = mesh.vertices[triangle[0] as usize];
                let b = mesh.vertices[triangle[1] as usize];
                let c = mesh.vertices[triangle[2] as usize];
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
                .intersect(primitive.clip_rect)
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
                        let tx = (uv.x * texture.size[0] as f32)
                            .floor()
                            .clamp(0.0, (texture.size[0] - 1) as f32)
                            as usize;
                        let ty = (uv.y * texture.size[1] as f32)
                            .floor()
                            .clamp(0.0, (texture.size[1] - 1) as f32)
                            as usize;
                        let tex = texture[(tx, ty)].to_array();
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
        image.save(path).unwrap();
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
    assert!(h.app.undo.is_some());
    h.app.undo_assignment();
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
fn all_fifteen_keys_remain_visible_in_both_themes_and_window_sizes() {
    for (size, label, dark) in [
        (vec2(1380.0, 900.0), "preview-dark.png", true),
        (vec2(1060.0, 720.0), "preview-small-dark.png", true),
        (vec2(1380.0, 900.0), "preview-light.png", false),
        (vec2(1060.0, 720.0), "preview-small-light.png", false),
    ] {
        let mut h = Harness::new();
        h.app.preferences.dark = dark;
        theme::apply(&h.ctx, dark);
        let output = h.warm(size);
        assert_eq!(h.app.drop_targets.len(), 15);
        for (r, i) in &h.app.drop_targets {
            assert!(
                r.width() > 55.0 && (r.width() - r.height()).abs() < 0.5,
                "Key {} clipped in {}: {:?}",
                i + 1,
                label,
                r
            );
        }
        if let Some(dir) = std::env::var_os("DECKFORGE_UI_OUTPUT") {
            std::fs::create_dir_all(&dir).unwrap();
            h.png(size, output, &std::path::PathBuf::from(dir).join(label));
        }
    }
}
#[test]
fn imports_real_export_when_reference_is_provided() {
    if let Some(path) = std::env::var_os("DECKFORGE_ELGATO_REFERENCE") {
        let report = crate::elgato::load(&PathBuf::from(path)).unwrap();
        assert!(report.converted > 0);
        if let Some(dir) = std::env::var_os("DECKFORGE_UI_OUTPUT") {
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
