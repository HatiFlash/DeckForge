//! Profil « NovyDeck Demo / README » : les 15 touches du D6 remplies avec des actions
//! existantes, pour la découverte et les captures de documentation. Il est ajouté à côté des
//! profils de l'utilisateur, sans jamais les modifier, et se réinitialise par son seul nom.
//! Les médias sont générés localement (aucun fichier tiers) et clairement nommés « demo ».
use crate::model::{Key, Page, Profile, Step};
use anyhow::Result;
use image::{Rgba, RgbaImage};
use std::path::{Path, PathBuf};

pub const NAME: &str = "NovyDeck Demo / README";
const FOLDER_ID: &str = "demo-dossier";
const PAGE_ID: &str = "demo-page-2";

/// Écrit (ou réécrit) les deux médias de démonstration dans `dir`.
pub fn write_media(dir: &Path) -> Result<(PathBuf, PathBuf)> {
    std::fs::create_dir_all(dir)?;
    let photo = dir.join("demo-paysage.png");
    let animation = dir.join("demo-egaliseur.gif");
    landscape(256).save(&photo)?;
    equalizer(&animation, 126)?;
    Ok((photo, animation))
}

fn key(label: &str, steps: Vec<Step>) -> Key {
    Key { label: label.into(), steps, ..Key::default() }
}

/// Profil complet. `photo` et `animation` viennent de [`write_media`].
pub fn profile(photo: &Path, animation: &Path) -> Profile {
    let mut p = Profile::new(NAME.into());
    p.keys = vec![
        key("Exemple macro", vec![
            Step::Delay { ms: 300 },
            Step::Hotkey { keys: "CTRL+C".into() },
            Step::Delay { ms: 150 },
            Step::Hotkey { keys: "CTRL+V".into() },
            Step::Notify { title: "NovyDeck".into(), message: "Copier-coller terminé".into() },
        ]),
        key("Lecture", vec![Step::Media { command: "play_pause".into() }]),
        key("Volume +", vec![Step::Media { command: "volume_up".into() }]),
        key("Volume -", vec![Step::Media { command: "volume_down".into() }]),
        key("Muet", vec![Step::Media { command: "mute".into() }]),
        key("Bloc-notes", vec![Step::Launch { program: "notepad.exe".into(), args: vec![], focus_if_running: true }]),
        key("Scène Démo", vec![Step::ObsScene { name: "Démo".into() }]),
        key("Dossier", vec![Step::Folder { id: FOLDER_ID.into() }]),
        Key { image: photo.to_string_lossy().into_owned(), ..key("Photo", vec![Step::Url { url: "https://github.com/HatiFlash/NovyDeck".into() }]) },
        Key { image: animation.to_string_lossy().into_owned(), ..key("", vec![Step::Media { command: "next".into() }]) },
        key("Gestionnaire", vec![Step::Hotkey { keys: "CTRL+SHIFT+ESC".into() }]),
        key("Signature", vec![Step::Text { text: "Cordialement,\nL'équipe NovyDeck".into() }]),
        key("Notifier", vec![Step::Notify { title: "NovyDeck".into(), message: "Touche de démonstration".into() }]),
        key("Micro", vec![Step::DeviceVolume { input: true, device: "default".into(), action: "toggle_mute".into(), step: 5, level: 50 }]),
        key("Page 2", vec![Step::PageNext]),
    ];
    let mut folder = Page::new(FOLDER_ID.into(), "Dossier démo".into(), true);
    folder.keys[1] = key("Calculatrice", vec![Step::Launch { program: "calc.exe".into(), args: vec![], focus_if_running: true }]);
    folder.keys[2] = key("Site NovyDeck", vec![Step::Url { url: "https://github.com/HatiFlash/NovyDeck".into() }]);
    let mut page = Page::new(PAGE_ID.into(), "Page 2".into(), false);
    page.keys[0] = key("Page 1", vec![Step::PagePrevious]);
    page.keys[1] = key("Précédent", vec![Step::Media { command: "previous".into() }]);
    page.keys[2] = key("Suivant", vec![Step::Media { command: "next".into() }]);
    page.keys[3] = key("Stop", vec![Step::Media { command: "stop".into() }]);
    p.pages = vec![page, folder];
    p
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Paysage stylisé (ciel, soleil, deux chaînes de montagnes, lac) : image de démonstration.
fn landscape(size: u32) -> RgbaImage {
    let s = size as f32;
    RgbaImage::from_fn(size, size, |x, y| {
        let (fx, fy) = (x as f32 / s, y as f32 / s);
        let mut c = mix([255.0, 176.0, 120.0], [70.0, 92.0, 170.0], (1.0 - fy * 1.6).clamp(0.0, 1.0));
        let sun = ((fx - 0.66).powi(2) + (fy - 0.42).powi(2)).sqrt();
        if sun < 0.11 { c = mix([255.0, 236.0, 190.0], c, (sun / 0.11).powi(4)); }
        let far = 0.55 - 0.10 * (fx * 7.0).sin().abs() - 0.04 * (fx * 17.0 + 1.0).sin();
        if fy > far { c = mix([96.0, 92.0, 150.0], [70.0, 70.0, 120.0], (fy - far) * 3.0); }
        let near = 0.68 - 0.08 * (fx * 4.0 + 0.6).sin() - 0.03 * (fx * 11.0).cos();
        if fy > near { c = mix([46.0, 58.0, 92.0], [28.0, 36.0, 62.0], (fy - near) * 4.0); }
        if fy > 0.80 {
            let ripple = ((fy * 90.0).sin() * 0.5 + 0.5) * 0.12;
            c = mix([52.0, 84.0, 140.0], [255.0, 196.0, 150.0], ripple + if (fx - 0.66).abs() < 0.05 { 0.25 } else { 0.0 });
        }
        Rgba([c[0] as u8, c[1] as u8, c[2] as u8, 255])
    })
}

/// Égaliseur animé (12 images), en boucle : animation de démonstration.
fn equalizer(path: &Path, size: u32) -> Result<()> {
    use image::codecs::gif::{GifEncoder, Repeat};
    let mut encoder = GifEncoder::new(std::fs::File::create(path)?);
    encoder.set_repeat(Repeat::Infinite)?;
    let bars = 7;
    for frame in 0..12 {
        let t = frame as f32 / 12.0 * std::f32::consts::TAU;
        let img = RgbaImage::from_fn(size, size, |x, y| {
            let (fx, fy) = (x as f32 / size as f32, y as f32 / size as f32);
            let mut c = mix([20.0, 28.0, 48.0], [34.0, 22.0, 60.0], fy);
            let slot = (fx * bars as f32).floor();
            let inside = (fx * bars as f32).fract();
            if (0.18..0.82).contains(&inside) && (0.12..0.88).contains(&fx) {
                let h = 0.25 + 0.5 * (0.5 + 0.5 * (t + slot * 0.9).sin()) * (0.6 + 0.4 * (slot * 1.7).cos().abs());
                if fy > 0.86 - h && fy < 0.86 { c = mix([52.0, 196.0, 238.0], [139.0, 92.0, 246.0], (0.86 - fy) / h); }
            }
            Rgba([c[0] as u8, c[1] as u8, c[2] as u8, 255])
        });
        encoder.encode_frame(image::Frame::from_parts(img, 0, 0, image::Delay::from_numer_denom_ms(90, 1)))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_profile_fills_the_fifteen_keys_with_valid_actions() {
        let dir = tempfile::tempdir().unwrap();
        let (photo, animation) = write_media(dir.path()).unwrap();
        assert!(photo.is_file() && animation.is_file());
        let p = profile(&photo, &animation);
        assert_eq!(p.name, NAME);
        assert_eq!(p.keys.len(), 15);
        let mut config = crate::model::Config::default();
        config.profiles.push(p.clone());
        for (i, k) in p.keys.iter().enumerate() {
            assert!(!k.steps.is_empty(), "touche {i} sans action");
            crate::model::validate_key_in_profile(k, &config, &p).unwrap_or_else(|e| panic!("touche {i} : {e}"));
        }
        for page in &p.pages {
            for k in page.keys.iter().filter(|k| !k.steps.is_empty()) {
                crate::model::validate_key_in_profile(k, &config, &p).unwrap();
            }
        }
        assert!(p.keys.iter().all(|k| !k.steps.iter().any(|s| matches!(s, Step::AutoHotkey { .. } | Step::HttpRequest { .. }))), "aucun script ni requête réseau");
        let gif = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(std::fs::File::open(&animation).unwrap())).unwrap();
        assert_eq!(image::AnimationDecoder::into_frames(gif).count(), 12);
    }
}
