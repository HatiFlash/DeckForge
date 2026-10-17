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
                .texture(&h.ctx, index, &key, h.app.config.image_size)
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
        if let Some(dir) = std::env::var_os("DECKFORGE_UI_OUTPUT") {
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
        .texture(&h.ctx, 5, &key, 126)
        .unwrap()
        .id();
    image::RgbImage::from_pixel(49, 23, image::Rgb([12, 220, 80]))
        .save(&file)
        .unwrap();
    let new = h
        .app
        .key_previews
        .texture(&h.ctx, 5, &key, 126)
        .unwrap()
        .id();
    assert_ne!(old, new);
    assert_eq!(
        new,
        h.app
            .key_previews
            .texture(&h.ctx, 5, &key, 126)
            .unwrap()
            .id()
    );
    let mut changed = key.clone();
    changed.label = "Nouveau titre".into();
    let label = h
        .app
        .key_previews
        .texture(&h.ctx, 5, &changed, 126)
        .unwrap()
        .id();
    assert_ne!(new, label);
    changed.color = [12, 34, 56];
    let color = h
        .app
        .key_previews
        .texture(&h.ctx, 5, &changed, 126)
        .unwrap()
        .id();
    assert_ne!(label, color);
    changed.background = false;
    let black = h
        .app
        .key_previews
        .texture(&h.ctx, 5, &changed, 126)
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
        .texture(&h.ctx, 5, &changed, 126)
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
            .texture(&h.ctx, 5, &changed, 126)
            .unwrap()
            .id()
    );
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
fn workflow_preview_shows_conditional_blocks() {
    let mut h = Harness::new();
    let size = vec2(1380.0, 900.0);
    h.app.config.keys_mut()[0] = Key {
        label: "Mode de travail".into(),
        steps: vec![
            Step::SetVariable {
                name: "mode".into(),
                value: "travail".into(),
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
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "Construis ton automatisation").is_some());
    assert!(text_position(&output.shapes, "BIBLIOTHÈQUE DE BLOCS").is_some());
    assert!(text_position(&output.shapes, "Clavier").is_some());
    if let Some(dir) = std::env::var_os("DECKFORGE_UI_OUTPUT") {
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
        .any(|s| matches!(&s.shape,egui::Shape::Circle(c) if c.radius==8.0 && c.fill==BLUE)));
    assert!(h.app.icon_notice.contains("appliquée"));
    assert!(h.app.icon_notice.contains("08"));
    if let Some(dir) = std::env::var_os("DECKFORGE_UI_OUTPUT") {
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
    let output = h.warm(size);
    assert!(text_position(&output.shapes, "14 px").is_some(), "title size is measured in pixels");
    assert!(text_position(&output.shapes, "Taille du titre").is_some());
    let origin = text_position(&output.shapes, "Titre sur le D6").unwrap();
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
        if let Some(dir) = std::env::var_os("DECKFORGE_UI_OUTPUT") {
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