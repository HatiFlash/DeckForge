//! Icônes d'interface : pictogrammes IconPark (ByteDance, licence Apache 2.0) en mode multi-couleur.
//!
//! Seuls les SVG réellement utilisés sont embarqués (`res/iconpark/`), sous forme de modèles
//! officiels du paquet `@icon-park/svg` dont les 4 couleurs du thème « multi-color » sont des jetons :
//! `@0@` contour extérieur, `@1@` remplissage extérieur, `@2@` contour intérieur, `@3@` remplissage intérieur.
//!
//! Chaque icône est rastérisée une seule fois par (icône, famille, thème, accent, taille en pixels
//! physiques) puis gardée en texture : rien n'est relu ni rastérisé à chaque image egui.
//! Ces icônes ne concernent que l'interface du PC : le rendu des touches du périphérique
//! (`icons.rs`, `key_render.rs`) et les images choisies par l'utilisateur restent inchangés.
use crate::model::Step;
use eframe::egui;
use egui::{Color32, TextureHandle};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Familles de couleurs, adaptées au thème sombre (et au thème clair).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    /// Clavier, souris, texte : orange doux.
    Input,
    /// Applications et fenêtres : bleu.
    App,
    /// Web et réseau : bleu / turquoise.
    Web,
    /// Conditions, attentes, répétitions, variables : violet / turquoise.
    Logic,
    /// Lecture, vidéo, sons : violet / rose.
    Media,
    /// Volume, micro, sorties audio : cyan / bleu.
    Audio,
    /// Réglages, périphérique, notifications : bleu acier.
    System,
    /// OBS, enregistrement, diffusion : rouge corail.
    Streaming,
    /// Plugins et magasin : vert / bleu.
    Plugin,
    /// Assistant IA et MCP : violet / bleu.
    Ai,
    /// Images, apparence, thèmes : rose / ambre.
    Image,
    /// Navigation et bibliothèque : couleur d'accent choisie par l'utilisateur.
    Accent,
    Warning,
    Danger,
    /// Petites commandes techniques (annuler, rétablir…) : une seule couleur, celle du texte.
    Mono,
}

macro_rules! icons {
    ($($variant:ident => $file:literal, $family:ident;)*) => {
        /// Pictogrammes disponibles. Ajouter une icône : copier le modèle dans `res/iconpark/` puis une ligne ici.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Ico { $($variant),* }
        impl Ico {
            #[cfg(test)]
            pub const ALL: &'static [Ico] = &[$(Ico::$variant),*];
            /// Nom officiel IconPark.
            pub fn name(self) -> &'static str { match self { $(Ico::$variant => $file),* } }
            /// Famille de couleurs par défaut.
            pub fn family(self) -> Family { match self { $(Ico::$variant => Family::$family),* } }
            fn template(self) -> &'static str {
                match self { $(Ico::$variant => include_str!(concat!("../res/iconpark/", $file, ".svg"))),* }
            }
        }
    };
}

icons! {
    // Clavier, souris, texte
    Keyboard => "keyboard", Input;
    Mouse => "mouse", Input;
    Text => "text", Input;
    Clipboard => "clipboard", Input;
    // Applications, fenêtres, web
    Application => "application-one", App;
    Window => "app-switch", App;
    CloseWindow => "close-one", App;
    WebPage => "web-page", Web;
    Network => "api", Web;
    // Logique et automatisations
    Delay => "hourglass-full", Logic;
    Wait => "stopwatch", Logic;
    Condition => "branch-one", Logic;
    Repeat => "cycle-one", Logic;
    Toggle => "switch-one", Logic;
    MultiAction => "all-application", Logic;
    Variable => "data-sheet", Logic;
    // Médias et audio
    Play => "play", Media;
    Pause => "pause-one", Media;
    Next => "go-end", Media;
    Previous => "go-start", Media;
    Music => "music", Media;
    Video => "video-one", Media;
    VolumeUp => "volume-up", Audio;
    VolumeDown => "volume-down", Audio;
    Mute => "volume-mute", Audio;
    Headset => "headset", Audio;
    Microphone => "microphone-one", Audio;
    Speaker => "speaker", Audio;
    // Système et périphérique
    Notify => "remind", System;
    Brightness => "brightness", System;
    Device => "devices", System;
    Settings => "setting-two", System;
    Advanced => "setting-one", System;
    Diagnostic => "stethoscope", System;
    Log => "log", System;
    // Streaming
    Camera => "videocamera", Streaming;
    Record => "record-disc", Streaming;
    Broadcast => "broadcast-radio", Streaming;
    // Plugins
    Plugin => "puzzle", Plugin;
    Store => "app-store", Plugin;
    Shop => "shopping-bag", Plugin;
    Plug => "plug-one", Plugin;
    Install => "install", Plugin;
    // Assistant IA et MCP
    Robot => "robot-one", Ai;
    Brain => "brain", Ai;
    Connection => "connect", Ai;
    // Images et apparence
    Palette => "platte", Image;
    Picture => "picture", Image;
    Album => "picture-album", Image;
    Sun => "sun-one", Image;
    Moon => "moon", Image;
    // Navigation, profils, pages, bibliothèque
    Actions => "lightning", Accent;
    Bookmark => "bookmark", Accent;
    Template => "page-template", Accent;
    Workshop => "blocks-and-arrows", Accent;
    Profile => "layers", Accent;
    Page => "doc-detail", Accent;
    Folder => "folder-close", Accent;
    Add => "add-one", Accent;
    Save => "save", Accent;
    Delete => "delete", Danger;
    Edit => "edit", Accent;
    Stop => "square", Media;
    PageNext => "right-c", Accent;
    PagePrevious => "left-c", Accent;
    Back => "arrow-circle-left", Accent;
    // États
    Warning => "attention", Warning;
    Error => "caution", Danger;
    Info => "info", Accent;
    // Commandes techniques monochromes
    Undo => "undo", Mono;
    Redo => "redo", Mono;
}

const WHITE: Color32 = Color32::WHITE;

fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// Les 4 couleurs IconPark (contour, remplissage, contour intérieur, remplissage intérieur).
///
/// Sur fond sombre le contour est une teinte claire de la famille (le `#333` d'origine disparaîtrait) ;
/// sur fond clair, une teinte foncée. Le remplissage reste moyen pour garder le contour intérieur blanc lisible.
pub fn colors(family: Family, dark: bool, accent: Color32, ink: Color32) -> [Color32; 4] {
    let (fill, inner) = match family {
        Family::Input => (rgb(0xE8873A), rgb(0xFFD08A)),
        Family::App => (rgb(0x3478F6), rgb(0x8FC2FF)),
        Family::Web => (rgb(0x1F8FD6), rgb(0x5EE0D0)),
        Family::Logic => (rgb(0x7B61E8), rgb(0x3FD6C6)),
        Family::Media => (rgb(0xA04BE0), rgb(0xFF7EB6)),
        Family::Audio => (rgb(0x1E9BD7), rgb(0x7FE3FF)),
        Family::System => (rgb(0x5B7896), rgb(0xA9C4DE)),
        Family::Streaming => (rgb(0xE5484D), rgb(0xFF9F7A)),
        Family::Plugin => (rgb(0x22A86B), rgb(0x6CC6FF)),
        Family::Ai => (rgb(0x6C5CE7), rgb(0x74B9FF)),
        Family::Image => (rgb(0xF0567A), rgb(0xFFC35C)),
        Family::Accent => (accent, mix(accent, WHITE, 0.55)),
        Family::Warning => (rgb(0xF5AD42), rgb(0xFFE2A8)),
        Family::Danger => (rgb(0xF25F70), rgb(0xFFC2C9)),
        Family::Mono => return [ink, Color32::TRANSPARENT, ink, Color32::TRANSPARENT],
    };
    let line = if dark { mix(fill, WHITE, 0.62) } else { mix(fill, Color32::BLACK, 0.45) };
    [line, fill, WHITE, inner]
}

fn css(c: Color32) -> String {
    if c.a() == 0 {
        "none".into()
    } else {
        format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
    }
}

/// Remplace les jetons du modèle par des couleurs.
pub fn svg(ico: Ico, colors: [Color32; 4]) -> String {
    let mut text = ico.template().to_owned();
    for (i, c) in colors.iter().enumerate() {
        text = text.replace(&format!("@{i}@"), &css(*c));
    }
    text
}

/// Rastérise une icône carrée de `px` pixels (RGBA prémultiplié, comme egui).
pub fn rasterize(ico: Ico, colors: [Color32; 4], px: u32) -> Option<egui::ColorImage> {
    let mut options = resvg::usvg::Options::default();
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    options.image_href_resolver.resolve_data = Box::new(|_, _, _| None);
    let tree = resvg::usvg::Tree::from_str(&svg(ico, colors), &options).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
    let scale = px as f32 / tree.size().width().max(tree.size().height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Some(egui::ColorImage::from_rgba_premultiplied([px as usize, px as usize], pixmap.data()))
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct CacheKey {
    ico: Ico,
    family: Family,
    dark: bool,
    accent: [u8; 4],
    ink: [u8; 4],
    px: u32,
}

#[derive(Default)]
struct Cache {
    textures: HashMap<CacheKey, Option<TextureHandle>>,
    rasterized: usize,
}

type Shared = Arc<Mutex<Cache>>;

fn cache(ctx: &egui::Context) -> Shared {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Shared>(egui::Id::new("deckforge_iconset")).clone())
}

/// Nombre d'icônes rastérisées depuis le démarrage (pour vérifier qu'aucune ne l'est à chaque image).
#[cfg(test)]
pub fn rasterized(ctx: &egui::Context) -> usize {
    cache(ctx).lock().map(|c| c.rasterized).unwrap_or(0)
}

/// Texture d'une icône de `size` points, nette à la densité de l'écran. `None` si le SVG est illisible.
pub fn texture(ctx: &egui::Context, ico: Ico, family: Family, size: f32) -> Option<TextureHandle> {
    let px = (size * ctx.pixels_per_point()).round().clamp(8.0, 256.0) as u32;
    let p = crate::theme::palette(ctx);
    let dark = ctx.style().visuals.dark_mode;
    let key = CacheKey { ico, family, dark, accent: p.accent.to_array(), ink: p.ink.to_array(), px };
    let shared = cache(ctx);
    let mut cache = shared.lock().ok()?;
    if let Some(found) = cache.textures.get(&key) {
        return found.clone();
    }
    cache.rasterized += 1;
    let texture = rasterize(ico, colors(family, dark, p.accent, p.ink), px).map(|image| {
        ctx.load_texture(format!("iconpark-{}-{px}", ico.name()), image, egui::TextureOptions::LINEAR)
    });
    cache.textures.insert(key, texture.clone());
    texture
}

/// Image egui prête à placer dans un bouton (`Button::image_and_text`) ou un `ui.add`.
pub fn image(ctx: &egui::Context, ico: Ico, size: f32) -> Option<egui::Image<'static>> {
    image_in(ctx, ico, ico.family(), size)
}

pub fn image_in(ctx: &egui::Context, ico: Ico, family: Family, size: f32) -> Option<egui::Image<'static>> {
    let texture = texture(ctx, ico, family, size)?;
    Some(egui::Image::from_texture(egui::load::SizedTexture::new(texture.id(), egui::vec2(size, size))))
}

/// Peint une icône centrée sur `center`. `strength` < 1 l'atténue (élément non sélectionné).
/// Repli : l'ancien symbole vectoriel `fallback` si l'icône ne peut pas être rendue.
pub fn paint(painter: &egui::Painter, center: egui::Pos2, size: f32, ico: Ico, family: Family, strength: f32, fallback: usize) {
    let rect = egui::Rect::from_center_size(center, egui::vec2(size, size));
    match texture(painter.ctx(), ico, family, size) {
        Some(texture) => {
            let tint = Color32::WHITE.gamma_multiply(strength.clamp(0.0, 1.0));
            painter.image(texture.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), tint);
        }
        None => crate::theme::symbol(painter, center, size * 0.85, fallback, crate::theme::accent()),
    }
}

/// Bouton secondaire (barre supérieure) avec icône ; le texte reste identique pour les tests et les lecteurs d'écran.
pub fn secondary_button(ctx: &egui::Context, ico: Ico, text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    let p = crate::theme::palette(ctx);
    match image(ctx, ico, 16.0) {
        Some(image) => egui::Button::image_and_text(image, text),
        None => egui::Button::new(text),
    }
    .fill(p.card)
    .stroke(egui::Stroke::new(1.0_f32, p.border))
    .corner_radius(8)
}

/// Bouton ordinaire avec icône.
pub fn button(ctx: &egui::Context, ico: Ico, text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    match image(ctx, ico, 16.0) {
        Some(image) => egui::Button::image_and_text(image, text),
        None => egui::Button::new(text),
    }
}

/// Menu déroulant avec icône (barre supérieure).
pub fn menu<R>(ui: &mut egui::Ui, ico: Ico, title: &str, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<Option<R>> {
    match image(ui.ctx(), ico, 16.0) {
        Some(image) => ui.menu_image_text_button(image, title, add_contents),
        None => ui.menu_button(title, add_contents),
    }
}

/// Onglet sélectionnable avec icône (atténuée quand l'onglet n'est pas actif).
pub fn tab(ui: &mut egui::Ui, selected: bool, ico: Ico, text: impl Into<egui::WidgetText>) -> egui::Response {
    let ctx = ui.ctx().clone();
    let button = match image(&ctx, ico, 16.0) {
        Some(image) => egui::Button::image_and_text(image.tint(Color32::WHITE.gamma_multiply(if selected { 1.0 } else { 0.8 })), text),
        None => egui::Button::new(text),
    };
    // Coins arrondis comme le fond des onglets (sinon le bleu de sélection reste carré).
    ui.add(button.selected(selected).frame(selected).corner_radius(TAB_RADIUS))
}

/// Rayon des onglets sélectionnés (fond des groupes d'onglets : 10, marge 3).
pub const TAB_RADIUS: u8 = 7;

/// Onglet sans icône, même forme que [`tab`].
pub fn plain_tab(ui: &mut egui::Ui, selected: bool, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(egui::Button::new(text).selected(selected).frame(selected).corner_radius(TAB_RADIUS))
}

/// Titre de section avec icône.
pub fn heading(ui: &mut egui::Ui, ico: Ico, text: impl Into<egui::WidgetText>) {
    ui.horizontal(|ui| {
        if let Some(image) = image(ui.ctx(), ico, 20.0) {
            ui.add(image);
        }
        ui.label(text);
    });
}

/// Icône d'interface d'un bloc d'action (indépendante de l'icône affichée sur la touche).
pub fn for_step(step: &Step) -> Ico {
    match step.active() {
        Step::Disabled { .. } => Ico::Delay,
        Step::Delay { .. } => Ico::Delay,
        Step::WaitWindow { .. } | Step::WaitUntil { .. } => Ico::Wait,
        Step::Hotkey { .. } | Step::KeyHold { .. } | Step::AutoHotkey { .. } => Ico::Keyboard,
        Step::Text { .. } | Step::TextOp { .. } => Ico::Text,
        Step::Clipboard { .. } => Ico::Clipboard,
        Step::MouseClick { .. } | Step::MouseMove { .. } | Step::MouseScroll { .. } => Ico::Mouse,
        Step::Launch { .. } => Ico::Application,
        Step::Url { .. } => Ico::WebPage,
        Step::UdpSend { .. } | Step::HttpRequest { .. } => Ico::Network,
        Step::WindowOp { .. } => Ico::Window,
        Step::CloseWindow => Ico::CloseWindow,
        Step::Media { command } => match command.as_str() {
            "volume_up" => Ico::VolumeUp,
            "volume_down" => Ico::VolumeDown,
            "mute" => Ico::Mute,
            "next" => Ico::Next,
            "previous" => Ico::Previous,
            "stop" => Ico::Stop,
            _ => Ico::Play,
        },
        Step::AppVolume { .. } | Step::DeviceVolume { input: false, .. } => Ico::Speaker,
        Step::DeviceVolume { input: true, .. } => Ico::Microphone,
        Step::AudioSwitch { input: true, .. } => Ico::Microphone,
        Step::AudioSwitch { .. } => Ico::Headset,
        Step::PlaySound { .. } | Step::StopSounds => Ico::Music,
        Step::Notify { .. } => Ico::Notify,
        Step::IfCondition { .. } | Step::IfWindow { .. } | Step::IfVariable { .. } => Ico::Condition,
        Step::Repeat { .. } => Ico::Repeat,
        Step::Toggle { .. } => Ico::Toggle,
        Step::SetVariable { .. } | Step::Math { .. } => Ico::Variable,
        Step::Profile { .. } => Ico::Profile,
        Step::Folder { .. } => Ico::Folder,
        Step::Page { .. } => Ico::Page,
        Step::PageNext => Ico::PageNext,
        Step::PagePrevious => Ico::PagePrevious,
        Step::Back => Ico::Back,
        Step::D6Brightness { .. } | Step::D6Sleep => Ico::Brightness,
        Step::ObsRecord { .. } | Step::ObsReplay { .. } => Ico::Record,
        Step::ObsStream { .. } => Ico::Broadcast,
        Step::ObsVirtualCam { .. } | Step::ObsScene { .. } | Step::ObsSourceVisibility { .. } => Ico::Camera,
        Step::ObsMute { .. } => Ico::Mute,
    }
}

/// Icône d'une touche ou d'une action de la bibliothèque : plusieurs blocs = multi-action.
pub fn for_steps(steps: &[Step]) -> Ico {
    match steps {
        [] => Ico::Add,
        [one] => for_step(one),
        _ => Ico::MultiAction,
    }
}

/// Famille d'une icône de bloc, utilisée aussi pour la barre de couleur de l'Atelier.
pub fn family_color(ctx: &egui::Context, ico: Ico) -> Color32 {
    let p = crate::theme::palette(ctx);
    colors(ico.family(), ctx.style().visuals.dark_mode, p.accent, p.ink)[1]
}

/// Texte de la licence IconPark, embarqué pour la fenêtre « À propos ».
pub const LICENSE: &str = include_str!("../res/iconpark/LICENSE");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_embedded_parses_and_is_multicolor_or_mono() {
        for &ico in Ico::ALL {
            let template = ico.template();
            assert!(template.contains("viewBox=\"0 0 48 48\""), "{}", ico.name());
            let colors = colors(ico.family(), true, crate::theme::BLUE, WHITE);
            let image = rasterize(ico, colors, 32).unwrap_or_else(|| panic!("{} illisible", ico.name()));
            let opaque = image.pixels.iter().filter(|p| p.a() > 0).count();
            assert!(opaque > 40, "{} presque vide ({opaque} px)", ico.name());
            // Suivant / précédent : IconPark ne les dessine qu'au trait.
            if ico.family() != Family::Mono && !matches!(ico, Ico::Next | Ico::Previous) {
                // Multi-couleur : au moins un remplissage (extérieur ou intérieur).
                assert!(template.contains("@1@") || template.contains("@3@"), "{} n'a qu'un contour", ico.name());
            }
            assert!(!svg(ico, colors).contains('@'), "{} : jeton non remplacé", ico.name());
        }
        let mut names: Vec<_> = Ico::ALL.iter().map(|i| i.name()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), Ico::ALL.len(), "une icône est déclarée deux fois");
    }

    #[test]
    fn only_used_icons_are_shipped() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("res/iconpark");
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".svg").map(str::to_owned)
        }).collect();
        files.sort();
        let mut used: Vec<_> = Ico::ALL.iter().map(|i| i.name().to_owned()).collect();
        used.sort();
        assert_eq!(files, used, "res/iconpark doit contenir exactement les icônes déclarées");
        assert!(LICENSE.contains("Apache License") && dir.join("NOTICE.md").exists());
    }

    #[test]
    fn dark_theme_outlines_stay_visible_and_light_theme_outlines_are_dark() {
        let lum = |c: Color32| 0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32;
        for family in [Family::Input, Family::App, Family::Web, Family::Logic, Family::Media, Family::Audio, Family::System,
            Family::Streaming, Family::Plugin, Family::Ai, Family::Image, Family::Warning, Family::Danger] {
            let dark = colors(family, true, crate::theme::BLUE, WHITE);
            let light = colors(family, false, crate::theme::BLUE, Color32::BLACK);
            assert!(lum(dark[0]) > 150.0, "{family:?} : contour trop sombre sur fond sombre");
            assert!(lum(light[0]) < 120.0, "{family:?} : contour trop clair sur fond clair");
        }
    }

    #[test]
    fn textures_are_cached_per_size_and_theme_not_per_frame() {
        let ctx = egui::Context::default();
        let first = texture(&ctx, Ico::Keyboard, Family::Input, 18.0).expect("texture");
        for _ in 0..50 {
            let again = texture(&ctx, Ico::Keyboard, Family::Input, 18.0).unwrap();
            assert_eq!(again.id(), first.id());
        }
        assert_eq!(rasterized(&ctx), 1);
        let _bigger = texture(&ctx, Ico::Keyboard, Family::Input, 32.0).unwrap();
        ctx.set_pixels_per_point(2.0);
        let _ = ctx.run(Default::default(), |_| {});
        let sharp = texture(&ctx, Ico::Keyboard, Family::Input, 18.0).unwrap();
        assert_eq!(sharp.size(), [36, 36], "rendu net en 200 %");
        assert_eq!(rasterized(&ctx), 3);
    }

    #[test]
    fn every_block_and_preset_has_an_interface_icon() {
        for kind in 0..crate::automation::NAMES.len() {
            let _ = for_step(&crate::automation::step(kind));
        }
        for preset in crate::catalog::presets() {
            let _ = for_steps(&preset.key.steps);
        }
        assert_eq!(for_step(&Step::Media { command: "mute".into() }), Ico::Mute);
        assert_eq!(for_steps(&[Step::Delay { ms: 1 }, Step::CloseWindow]), Ico::MultiAction);
    }

    /// Planche de contrôle : `DECKFORGE_UI_OUTPUT=dossier cargo test iconset -- --ignored`.
    #[test]
    #[ignore = "écrit une planche PNG de toutes les icônes"]
    fn contact_sheet() {
        let dir = std::env::var_os("DECKFORGE_UI_OUTPUT").expect("DECKFORGE_UI_OUTPUT");
        for (dark, bg, name) in [(true, [16u8, 24, 39], "iconset-sombre.png"), (false, [244, 246, 250], "iconset-clair.png")] {
            let cols = 12;
            let cell = 64u32;
            let rows = Ico::ALL.len().div_ceil(cols) as u32;
            let mut sheet = image::RgbaImage::from_pixel(cols as u32 * cell, rows * cell, image::Rgba([bg[0], bg[1], bg[2], 255]));
            let ink = if dark { Color32::from_rgb(230, 236, 245) } else { Color32::from_rgb(30, 36, 48) };
            for (i, &ico) in Ico::ALL.iter().enumerate() {
                let (x0, y0) = ((i % cols) as u32 * cell, (i / cols) as u32 * cell);
                for (px, dx, dy) in [(32u32, 4u32, 4u32), (16, 42, 42)] {
                    let img = rasterize(ico, colors(ico.family(), dark, crate::theme::BLUE, ink), px).unwrap();
                    for y in 0..px {
                        for x in 0..px {
                            let c = img.pixels[(y * px + x) as usize];
                            let a = c.a() as u32;
                            let o = sheet.get_pixel_mut(x0 + dx + x, y0 + dy + y);
                            for k in 0..3 {
                                o.0[k] = ((c.to_array()[k] as u32 + o.0[k] as u32 * (255 - a) / 255).min(255)) as u8;
                            }
                        }
                    }
                }
            }
            sheet.save(std::path::Path::new(&dir).join(name)).unwrap();
        }
    }
}
