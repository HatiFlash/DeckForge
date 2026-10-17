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
pub fn import_image(bytes: &[u8], dest: &Path, name: &str) -> Result<PathBuf> {
    ensure!(bytes.len() <= 20_000_000, "Image trop volumineuse");
    let gif = bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a");
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
        else { let _ = image::load_from_memory_with_format(bytes, image::ImageFormat::Gif)?; std::fs::write(&path, bytes)?; }
    }
    Ok(path)
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
    for p in pixels.chunks_exact_mut(4) {
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
