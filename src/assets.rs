//! Local icon discovery and bounded package import. Plugins are never executed here.
use anyhow::{ensure, Result};
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read},
    path::{Path, PathBuf},
};
#[derive(Clone, Debug)]
pub struct Icon {
    pub pack: String,
    pub name: String,
    pub path: PathBuf,
}
#[derive(Clone, Debug)]
pub struct Plugin {
    pub name: String,
    pub id: String,
    pub path: PathBuf,
    pub actions: usize,
}
pub fn roots() -> Vec<PathBuf> {
    let mut paths = vec![];
    if let Some(p) = std::env::var_os("ProgramFiles(x86)") {
        paths.push(PathBuf::from(p).join("fifine Control Deck/icons"));
    }
    if let Some(p) = std::env::var_os("APPDATA") {
        for suffix in ["HotSpot/StreamDock/icons", "Elgato/StreamDeck/IconPacks"] {
            paths.push(PathBuf::from(&p).join(suffix));
        }
    }
    paths
}
fn walk(root: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    if depth > 8 || files.len() >= 8000 {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            if files.len() >= 8000 {
                break;
            }
            let p = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&p) else {
                continue;
            };
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                continue;
            }
            if meta.is_dir() {
                walk(&p, depth + 1, files);
            } else if meta.is_file() && meta.len() <= 8_000_000 {
                files.push(p);
            }
        }
    }
}
pub fn scan(extra: &[PathBuf]) -> Vec<Icon> {
    let mut icons = vec![];
    let mut seen = HashSet::new();
    for root in extra.iter().cloned().chain(roots()) {
        let mut labels = HashMap::<String, (String, HashMap<String, String>)>::new();
        let mut files = vec![];
        walk(&root, 0, &mut files);
        for path in files {
            if !matches!(
                path.extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .as_str(),
                "png" | "jpg" | "jpeg" | "svg" | "gif"
            ) {
                continue;
            }
            let relative = path.strip_prefix(&root).unwrap_or(&path);
            if !seen.insert(relative.to_string_lossy().to_lowercase()) {
                continue;
            }
            let folder = relative
                .components()
                .next()
                .map(|s| s.as_os_str().to_string_lossy().into_owned())
                .unwrap_or_else(|| "Mes icônes".into());
            let (pack, names) = labels.entry(folder.clone()).or_insert_with(|| {
                let folder_path = root.join(&folder);
                let mut pack = folder.clone();
                let mut names = HashMap::new();
                if let Some(v) = read_json(&folder_path.join("manifest.json")) {
                    if let Some(name) = v["Name"].as_str() {
                        pack = name.into();
                    }
                }
                if let Some(v) = read_json(&folder_path.join("icons.json")) {
                    if let Some(array) = v.as_array() {
                        for entry in array {
                            if let (Some(path), Some(name)) =
                                (entry["path"].as_str(), entry["name"].as_str())
                            {
                                names.insert(path.to_lowercase(), name.to_string());
                            }
                        }
                    }
                }
                (pack, names)
            });
            let name = names
                .get(
                    &path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase(),
                )
                .cloned()
                .unwrap_or_else(|| {
                    path.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                });
            icons.push(Icon {
                pack: pack.clone(),
                name,
                path,
            });
            if icons.len() >= 8000 {
                return icons;
            }
        }
    }
    icons.sort_by(|a, b| (&a.pack, &a.name).cmp(&(&b.pack, &b.name)));
    icons
}
fn read_json(path: &Path) -> Option<serde_json::Value> {
    if std::fs::metadata(path).ok()?.len() > 2_000_000 {
        return None;
    }
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}
fn normalized_gif(bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    use image::{AnimationDecoder, ImageDecoder};
    let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes))?;
    let (width, height) = decoder.dimensions();
    if width <= 512 && height <= 512 { return Ok(None); }
    ensure!(width <= 4096 && height <= 4096, "GIF trop grand pour une conversion sûre");
    let mut output = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut output);
        encoder.set_repeat(image::codecs::gif::Repeat::Infinite)?;
        let mut count = 0;
        let mut elapsed = 0u32;
        for frame in decoder.into_frames().take(151) {
            let frame = frame?;
            count += 1;
            ensure!(count <= 150, "GIF supérieur à 150 images");
            let (num, den) = frame.delay().numer_denom_ms();
            elapsed = elapsed.saturating_add(num.checked_div(den).unwrap_or(100).max(20));
            ensure!(elapsed <= 10_000, "GIF supérieur à 10 secondes");
            let image = image::DynamicImage::ImageRgba8(frame.buffer().clone()).thumbnail(256, 256).into_rgba8();
            encoder.encode_frame(image::Frame::from_parts(image, 0, 0, frame.delay()))?;
        }
    }
    ensure!(output.len() <= 20_000_000, "GIF converti trop volumineux");
    Ok(Some(output))
}
pub fn import_image(bytes: &[u8], dest: &Path, name: &str) -> Result<PathBuf> {
    ensure!(bytes.len() <= 20_000_000, "Image trop volumineuse");
    let gif = bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a");
    let normalized = if gif { normalized_gif(bytes)? } else { None };
    let decoded = if gif { None } else { Some(decode_image(bytes, 512)?) };
    let hash = bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ (*b as u64)).wrapping_mul(0x100000001b3)
    });
    let safe: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(60)
        .collect();
    std::fs::create_dir_all(dest)?;
    let path = dest.join(format!("{safe}-{hash:016x}.{}", if gif { "gif" } else { "png" }));
    if !path.exists() {
        if let Some(decoded) = decoded { decoded.save_with_format(&path, image::ImageFormat::Png)?; }
        else { let _ = image::load_from_memory_with_format(bytes, image::ImageFormat::Gif)?; std::fs::write(&path, normalized.as_deref().unwrap_or(bytes))?; }
    }
    Ok(path)
}
pub fn is_video(path: &Path) -> bool {
    matches!(path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase().as_str(),
        "mp4" | "webm" | "mov" | "mkv" | "avi")
}

pub fn ffmpeg_path() -> Option<PathBuf> {
    let name = "ffmpeg.exe";
    // Lets tests and support check the Windows decoder on a PC that has FFmpeg.
    if std::env::var_os("DECKFORGE_NO_FFMPEG").is_some() { return None; }
    if let Some(own) = tools_dir().map(|d| d.join(name)).filter(|p| p.is_file()) { return Some(own); }
    if let Some(paths) = std::env::var_os("PATH") {
        for folder in std::env::split_paths(&paths) {
            let candidate = folder.join(name);
            if candidate.is_file() { return Some(candidate); }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        let candidate = exe.with_file_name(name);
        if candidate.is_file() { return Some(candidate); }
    }
    let winget = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Microsoft/WinGet");
    let link = winget.join("Links").join(name);
    if link.is_file() { return Some(link); }
    let packages = winget.join("Packages");
    for package in std::fs::read_dir(packages).ok()?.flatten() {
        if !package.file_name().to_string_lossy().starts_with("Gyan.FFmpeg") { continue; }
        if let Ok(versions) = std::fs::read_dir(package.path()) {
            for version in versions.flatten() {
                let candidate = version.path().join("bin/ffmpeg.exe");
                if candidate.is_file() { return Some(candidate); }
            }
        }
    }
    None
}

pub const FFMPEG_MISSING: &str = "FFmpeg absent";

const FFMPEG_URL: &str = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";

/// Folder where DeckForge keeps its own copy of FFmpeg (per user, always writable).
pub fn tools_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("DeckForge").join("tools"))
}

fn hidden(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    let mut command = std::process::Command::new(program);
    command.creation_flags(0x0800_0000).stdin(std::process::Stdio::null());
    command
}

/// Runs a download command with a time limit; never opens a console window.
fn wait_download(mut command: std::process::Command, seconds: u64) -> Result<()> {
    use std::time::{Duration, Instant};
    let mut child = command.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn()?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? { ensure!(status.success(), "téléchargement échoué ({status})"); return Ok(()); }
        if start.elapsed() >= Duration::from_secs(seconds) { let _ = child.kill(); let _ = child.wait(); anyhow::bail!("téléchargement interrompu après {seconds} s"); }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn download(url: &str, to: &Path, seconds: u64) -> Result<()> {
    let mut curl = hidden("curl.exe");
    curl.args(["-L", "--fail", "--silent", "--show-error", "--retry", "2", "-o"]).arg(to).arg(url);
    if wait_download(curl, seconds).is_ok() && to.is_file() { return Ok(()); }
    let mut ps = hidden("powershell.exe");
    ps.args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(format!("$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri '{url}' -OutFile '{}'", to.display()));
    wait_download(ps, seconds)
}

fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    loop { let n = file.read(&mut buffer)?; if n == 0 { break; } hash.update(&buffer[..n]); }
    Ok(format!("{:x}", hash.finalize()))
}

/// Downloads the official FFmpeg "essentials" build after an explicit click, checks its
/// published SHA-256 and keeps only ffmpeg.exe in %LOCALAPPDATA%\DeckForge\tools.
pub fn install_ffmpeg() -> Result<PathBuf> {
    let dir = tools_dir().ok_or_else(|| anyhow::anyhow!("dossier LOCALAPPDATA introuvable"))?;
    std::fs::create_dir_all(&dir)?;
    let archive = dir.join("ffmpeg-download.zip");
    let checksum = dir.join("ffmpeg-download.sha256");
    let result = (|| -> Result<PathBuf> {
        download(&format!("{FFMPEG_URL}.sha256"), &checksum, 60)
            .map_err(|e| anyhow::anyhow!("Impossible de joindre gyan.dev ({e}). Vérifie la connexion Internet."))?;
        let expected = std::fs::read_to_string(&checksum)?.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
        ensure!(expected.len() == 64, "Empreinte FFmpeg illisible");
        download(FFMPEG_URL, &archive, 1200).map_err(|e| anyhow::anyhow!("Téléchargement de FFmpeg impossible ({e})"))?;
        ensure!(sha256_file(&archive)? == expected, "Le fichier FFmpeg téléchargé est corrompu (empreinte différente) : réessaie");
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&archive)?)?;
        let index = (0..zip.len()).find(|&i| zip.by_index(i).is_ok_and(|f| f.name().is_ok_and(|n| n.replace('\\', "/").ends_with("/bin/ffmpeg.exe"))))
            .ok_or_else(|| anyhow::anyhow!("ffmpeg.exe absent de l'archive"))?;
        let temporary = dir.join("ffmpeg.exe.part");
        std::io::copy(&mut zip.by_index(index)?, &mut std::fs::File::create(&temporary)?)?;
        let target = dir.join("ffmpeg.exe");
        let _ = std::fs::remove_file(&target);
        std::fs::rename(&temporary, &target)?;
        Ok(target)
    })();
    let _ = std::fs::remove_file(archive);
    let _ = std::fs::remove_file(checksum);
    result
}

/// Appends one import attempt to diagnostics/import-video.log next to the configuration.
fn video_log(dest: &Path, line: &str) {
    use std::io::Write;
    let Some(root) = dest.parent().and_then(Path::parent) else { return };
    let folder = root.join("diagnostics");
    if std::fs::create_dir_all(&folder).is_err() { return; }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(folder.join("import-video.log")) {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let _ = writeln!(file, "[{secs}] DeckForge {} | {line}", env!("CARGO_PKG_VERSION"));
    }
}

pub fn import_video(src: &Path, dest: &Path, size: u32) -> Result<PathBuf> {
    let result = import_video_inner(src, dest, size);
    let ext = src.extension().and_then(|s| s.to_str()).unwrap_or("?");
    let bytes = std::fs::metadata(src).map_or(0, |m| m.len());
    match &result {
        Ok(path) => video_log(dest, &format!("OK .{ext} {bytes} octets -> {}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())),
        Err(e) => video_log(dest, &format!("ÉCHEC .{ext} {bytes} octets, taille {size} ; ffmpeg={:?} ; erreur : {e}", ffmpeg_path().map(|p| p.file_name().map(|n| n.to_owned())))),
    }
    result
}

/// Without FFmpeg: decode with Windows itself (MP4/MOV H.264 everywhere, other formats if
/// the matching Windows video extension is installed).
fn import_video_native(src: &Path, dest: &Path, size: u32) -> Result<PathBuf> {
    let frames = crate::video_mf::decode(src, 12, 10, size)
        .map_err(|e| anyhow::anyhow!("{FFMPEG_MISSING} : Windows n'a pas pu convertir cette vidéo ({e})"))?;
    let bytes = crate::video_mf::encode_gif(frames, 12)?;
    ensure!(bytes.len() <= 20_000_000, "GIF supérieur à 20 Mo");
    std::fs::create_dir_all(dest)?;
    let name = src.file_stem().and_then(|s| s.to_str()).unwrap_or("video");
    let imported = import_image(&bytes, dest, name)?;
    let key = crate::model::Key { image: imported.to_string_lossy().into_owned(), ..Default::default() };
    if let Err(e) = crate::key_render::font().and_then(|font| crate::key_render::animated(&key, size, &font)) {
        let _ = std::fs::remove_file(&imported);
        return Err(e);
    }
    Ok(imported)
}

fn import_video_inner(src: &Path, dest: &Path, size: u32) -> Result<PathBuf> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    ensure!(is_video(src) && src.is_file() && [112, 126].contains(&size), "Vidéo ou taille invalide");
    let Some(ffmpeg) = ffmpeg_path() else { return import_video_native(src, dest, size); };
    std::fs::create_dir_all(dest)?;
    let name = src.file_stem().and_then(|s| s.to_str()).unwrap_or("video");
    let temporary = dest.join(format!(".video-{}-{:?}.gif", std::process::id(), std::thread::current().id()));
    let errors = temporary.with_extension("err.txt");
    let filter = format!("fps=12,scale={size}:{size}:force_original_aspect_ratio=increase,crop={size}:{size},split[a][b];[a]palettegen=max_colors=128[p];[b][p]paletteuse");
    let result = (|| -> Result<PathBuf> {
        let mut child = Command::new(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-y", "-t", "10", "-i"])
            .arg(src).args(["-vf", &filter, "-loop", "0"])
            .arg(&temporary).creation_flags(0x08000000)
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(std::fs::File::create(&errors)?).spawn()?;
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    let detail = std::fs::read_to_string(&errors).unwrap_or_default();
                    let detail: String = detail.trim().chars().rev().take(300).collect::<Vec<_>>().into_iter().rev().collect();
                    anyhow::bail!("Conversion FFmpeg échouée ({status}) : {}", if detail.is_empty() { "aucun détail" } else { &detail });
                }
                break;
            }
            if start.elapsed() >= Duration::from_secs(60) {
                let _ = child.kill(); let _ = child.wait();
                anyhow::bail!("Conversion vidéo interrompue après 60 s");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let bytes = std::fs::read(&temporary)?;
        ensure!(bytes.len() <= 20_000_000, "GIF supérieur à 20 Mo");
        let imported = import_image(&bytes, dest, name)?;
        let key = crate::model::Key { image: imported.to_string_lossy().into_owned(), ..Default::default() };
        let valid = crate::key_render::font().and_then(|font| crate::key_render::animated(&key, size, &font));
        if let Err(e) = valid { let _ = std::fs::remove_file(&imported); return Err(e); }
        Ok(imported)
    })();
    let _ = std::fs::remove_file(temporary);
    let _ = std::fs::remove_file(errors);
    result
}

#[cfg(test)]
mod video_tests {
    use super::*;
    #[test]
    fn video_extension_and_optional_conversion() {
        assert!(is_video(Path::new("VIDEO.MP4")));
        assert!(is_video(Path::new("clip.webm")));
        assert!(!is_video(Path::new("image.gif")));
        let Some(ffmpeg) = ffmpeg_path() else { eprintln!("FFmpeg absent : essai réel omis"); return; };
        use std::os::windows::process::CommandExt;
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("test.mp4");
        let status = std::process::Command::new(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc=size=320x240:rate=25", "-t", "1"])
            .arg(&src).creation_flags(0x08000000).status().unwrap();
        assert!(status.success(), "génération de la vidéo d'essai");
        let gif = import_video(&src, &temp.path().join("assets"), 126).unwrap();
        assert!(gif.exists());
        let key = crate::model::Key { image: gif.to_string_lossy().into_owned(), ..Default::default() };
        assert!(crate::key_render::animated(&key, 126, &crate::key_render::font().unwrap()).unwrap().len() > 1);
    }
    /// Real 115 MB download; run explicitly with `--ignored`.
    #[test]
    #[ignore]
    fn ffmpeg_download_real() {
        let exe = install_ffmpeg().unwrap();
        assert!(exe.is_file() && std::fs::metadata(&exe).unwrap().len() > 10_000_000);
        let out = std::process::Command::new(&exe).arg("-version").output().unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).starts_with("ffmpeg version"));
    }
    #[test]
    fn windows_decoder_imports_h264_without_ffmpeg() {
        let temp = tempfile::tempdir().unwrap();
        let broken = temp.path().join("pas-une-video.mp4");
        std::fs::write(&broken, b"ceci n'est pas une video").unwrap();
        let error = import_video_native(&broken, &temp.path().join("assets"), 126).unwrap_err().to_string();
        assert!(error.contains(FFMPEG_MISSING), "{error}");
        // FFmpeg only builds the sample file; decoding below uses Windows alone.
        let Some(ffmpeg) = ffmpeg_path() else { eprintln!("FFmpeg absent : vidéo d'essai non générée"); return; };
        use std::os::windows::process::CommandExt;
        let src = temp.path().join("bandes.mp4");
        let status = std::process::Command::new(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i",
                "color=c=red:s=96x64:d=2:r=24,drawbox=x=0:y=32:w=96:h=32:color=blue:t=fill",
                "-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .arg(&src).creation_flags(0x08000000).status().unwrap();
        assert!(status.success(), "génération de la vidéo d'essai");
        let frames = crate::video_mf::decode(&src, 12, 10, 32).unwrap();
        assert!((20..=26).contains(&frames.len()), "{} images", frames.len());
        let (top, bottom) = (frames[0].get_pixel(16, 3).0, frames[0].get_pixel(16, 28).0);
        assert!(top[0] > 170 && top[2] < 90, "haut {top:?}");
        assert!(bottom[2] > 170 && bottom[0] < 90, "bas {bottom:?}");
        let gif = import_video_native(&src, &temp.path().join("assets"), 126).unwrap();
        let key = crate::model::Key { image: gif.to_string_lossy().into_owned(), ..Default::default() };
        assert!(crate::key_render::animated(&key, 126, &crate::key_render::font().unwrap()).unwrap().len() > 10);
    }
}

pub fn decode_image(bytes: &[u8], size: u32) -> Result<image::DynamicImage> {
    ensure!(
        bytes.len() <= 20_000_000 && (1..=512).contains(&size),
        "Image trop volumineuse"
    );
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    if reader.format().is_some() {
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        return Ok(reader.decode()?.thumbnail(size, size));
    }
    let mut options = resvg::usvg::Options::default();
    // Static vector paths only: never resolve local files, URLs or embedded images.
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    options.image_href_resolver.resolve_data = Box::new(|_, _, _| None);
    let tree = resvg::usvg::Tree::from_data(bytes, &options)?;
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    let width = (tree.size().width() * scale).ceil().max(1.0) as u32;
    let height = (tree.size().height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| anyhow::anyhow!("SVG trop volumineux"))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let mut pixels = pixmap.take();
    for p in pixels.as_chunks_mut::<4>().0 {
        let alpha = p[3] as u32;
        if alpha > 0 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255) / alpha).min(255) as u8;
            }
        }
    }
    Ok(image::DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(width, height, pixels).unwrap(),
    ))
}
pub fn import_pack(path: &Path, dest: &Path) -> Result<(usize, usize)> {
    ensure!(
        std::fs::metadata(path)?.len() <= 64 * 1024 * 1024,
        "Pack supérieur à 64 Mo"
    );
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    ensure!(zip.len() <= 4096, "Pack : trop de fichiers");
    let mut total = 0u64;
    let mut count = 0;
    let mut skipped = 0;
    // Validate the complete archive before creating files.
    for i in 0..zip.len() {
        let file = zip.by_index(i)?;
        ensure!(
            file.enclosed_name().is_some(),
            "Chemin non sûr dans le pack"
        );
        total = total
            .checked_add(file.size())
            .ok_or_else(|| anyhow::anyhow!("Pack trop volumineux"))?;
        ensure!(
            total <= 128 * 1024 * 1024,
            "Pack décompressé supérieur à 128 Mo"
        );
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let safe: String = stem
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(80)
        .collect();
    let folder = dest.join(format!("pack-{safe}"));
    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        let path = file.enclosed_name().unwrap();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "svg" | "gif") {
            if matches!(ext.as_str(), "gif" | "webp") {
                skipped += 1;
            }
            continue;
        }
        ensure!(file.size() <= 8_000_000, "Image trop volumineuse");
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let mut bytes = vec![];
        file.read_to_end(&mut bytes)?;
        match import_image(&bytes, &folder, &name) {
            Ok(_) => count += 1,
            Err(_) => skipped += 1,
        }
    }
    ensure!(count > 0, "Aucune image PNG/JPEG lisible dans ce pack");
    Ok((count, skipped))
}
pub fn plugins() -> Vec<Plugin> {
    let mut roots = vec![];
    if let Some(p) = std::env::var_os("APPDATA") {
        for suffix in ["Elgato/StreamDeck/Plugins", "HotSpot/StreamDock/plugins"] {
            roots.push(PathBuf::from(&p).join(suffix));
        }
    }
    if let Some(p) = std::env::var_os("ProgramFiles(x86)") {
        roots.push(PathBuf::from(p).join("fifine Control Deck/plugins"));
    }
    let mut plugins = vec![];
    for root in roots {
        if let Ok(entries) = std::fs::read_dir(root) {
            for e in entries.flatten().take(256) {
                let path = e.path();
                let manifest = path.join("manifest.json");
                if std::fs::metadata(&manifest).map_or(true, |m| m.len() > 2_000_000) {
                    continue;
                }
                if let Ok(bytes) = std::fs::read(manifest) {
                    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        plugins.push(Plugin {
                            name: v["Name"].as_str().unwrap_or("Plugin").into(),
                            id: v["UUID"].as_str().unwrap_or("").into(),
                            path,
                            actions: v["Actions"].as_array().map_or(0, Vec::len),
                        });
                    }
                }
            }
        }
    }
    plugins
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn svg_vectors_are_rasterized_at_bounded_size_without_resolving_images() {
        let svg=br##"<svg xmlns="http://www.w3.org/2000/svg" width="10000" height="20000"><rect width="10000" height="20000" fill="#ff0000"/><image href="file:///not-to-be-read.png" width="100" height="100"/></svg>"##;
        let image = decode_image(svg, 64).unwrap().into_rgba8();
        assert_eq!(image.dimensions(), (32, 64));
        assert_eq!(image.get_pixel(20, 40).0, [255, 0, 0, 255]);
        let d = tempfile::tempdir().unwrap();
        let path = import_image(svg, d.path(), "vector").unwrap();
        assert_eq!(path.extension().unwrap(), "png");
        assert!(image::open(path).is_ok());
    }
    #[test]
    fn icon_package_import_normalizes_images_and_rejects_traversal_before_writing() {
        let d = tempfile::tempdir().unwrap();
        let pack = d.path().join("test.zip");
        let dest = d.path().join("assets");
        let mut png = Cursor::new(vec![]);
        image::RgbImage::from_pixel(4, 4, image::Rgb([30, 80, 140]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&pack).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("icons/../outside.png", options).unwrap();
        zip.write_all(png.get_ref()).unwrap();
        zip.start_file("../../escape.png", options).unwrap();
        zip.write_all(png.get_ref()).unwrap();
        zip.finish().unwrap();
        assert!(import_pack(&pack, &dest).is_err());
        assert!(!dest.exists());
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&pack).unwrap());
        zip.start_file("icons/casque.png", options).unwrap();
        zip.write_all(png.get_ref()).unwrap();
        zip.start_file("icons/a.svg", options).unwrap();
        zip.write_all(b"<svg/>").unwrap();
        zip.finish().unwrap();
        assert_eq!(import_pack(&pack, &dest).unwrap(), (2, 0));
        let icons = scan(&[dest.clone()]);
        assert!(icons.iter().any(|i| i.path.starts_with(&dest)));
        assert!(!d.path().join("escape.png").exists());
    }
}

#[cfg(test)]
mod gif_size_tests {
    use super::*;
    use image::ImageDecoder;
    #[test]
    fn large_gif_is_reencoded_at_256_preserving_animation() {
        let mut bytes = Vec::new();
        {
            let mut writer = image::codecs::gif::GifEncoder::new(&mut bytes);
            for c in [image::Rgba([255, 0, 0, 255]), image::Rgba([0, 255, 0, 255])] {
                writer.encode_frame(image::Frame::from_parts(image::RgbaImage::from_pixel(600, 520, c), 0, 0, image::Delay::from_numer_denom_ms(100, 1))).unwrap();
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let path = import_image(&bytes, dir.path(), "large").unwrap();
        let stored = std::fs::read(path).unwrap();
        assert_ne!(stored, bytes);
        let decoded = image::codecs::gif::GifDecoder::new(Cursor::new(stored)).unwrap();
        let (w,h) = decoded.dimensions();
        assert!(w <= 256 && h <= 256);
        assert_eq!(image::AnimationDecoder::into_frames(decoded).collect_frames().unwrap().len(), 2);
    }
}