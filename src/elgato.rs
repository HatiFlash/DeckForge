//! Read-only conversion of exported profiles. No plugin code is loaded or executed.
use crate::model::{Config, Key, Page, Profile, Step};
use anyhow::{anyhow, bail, ensure, Context, Result};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::Path,
};

#[derive(Debug)]
pub struct Report {
    pub profile: Profile,
    pub warnings: Vec<String>,
    pub converted: usize,
    pub icons: Vec<(String, usize, Vec<u8>)>,
}
fn safe(name: &str) -> bool {
    !name.starts_with('/')
        && !name.contains('\0')
        && name.len() <= 1024
        && !name.contains(':')
        && !name.contains('\\')
        && !name.split('/').any(|p| p == "..")
}
#[allow(dead_code)]
pub fn load(path: &Path) -> Result<Report> {
    let reports = load_all(path)?;
    reports
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("Aucun profil Elgato valide trouvé"))
}
pub fn load_all(path: &Path) -> Result<Vec<Report>> {
    let data = std::fs::read(path)?;
    ensure!(data.len() <= 256 * 1024 * 1024, "Archive de plus de 256 Mo");
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
    {
        ensure!(data.len() <= 2 * 1024 * 1024, "Manifest trop volumineux");
        return Ok(vec![convert(HashMap::from([("manifest.json".into(), data)]))?]);
    }
    load_all_zip_bytes(&data)
}
fn load_all_zip_bytes(data: &[u8]) -> Result<Vec<Report>> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(data)).context("Profil ou sauvegarde Elgato ZIP invalide")?;
    ensure!(archive.len() <= 16384, "Archive contenant trop de fichiers");
    let mut total = 0u64;
    let mut nested_profiles = Vec::new();
    let mut files = HashMap::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let name = file.name()?.to_string();
        ensure!(safe(&name), "Chemin non sûr dans l’archive");
        total = total
            .checked_add(file.size())
            .ok_or_else(|| anyhow!("Archive trop volumineuse"))?;
        ensure!(
            total <= 512 * 1024 * 1024,
            "Archive décompressée de plus de 512 Mo"
        );
        if file.is_dir() {
            continue;
        }
        let lower = name.to_lowercase();
        if lower.ends_with(".streamdeckprofile") || lower.ends_with(".streamdeckaction") {
            let limit = 64 * 1024 * 1024;
            ensure!(file.size() <= limit, "Profil imbriqué trop volumineux");
            let mut bytes = Vec::new();
            file.take(limit + 1).read_to_end(&mut bytes)?;
            ensure!(bytes.len() <= limit as usize, "Profil imbriqué décompressé trop volumineux");
            nested_profiles.push((name, bytes));
            continue;
        }
        let json = lower.ends_with("manifest.json");
        let image = lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg");
        if !json && !image {
            continue;
        }
        let limit = if json {
            2 * 1024 * 1024
        } else {
            5 * 1024 * 1024
        };
        ensure!(
            file.size() <= limit,
            "Fichier trop volumineux dans le profil"
        );
        let mut bytes = Vec::new();
        file.take(limit + 1).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= limit as usize, "Fichier décompressé trop volumineux");
        ensure!(files.insert(name, bytes).is_none(), "Entrées ZIP en double");
    }
    if !nested_profiles.is_empty() {
        let mut reports = Vec::new();
        let mut errors = Vec::new();
        for (name, bytes) in nested_profiles {
            match load_all_zip_bytes(&bytes) {
                Ok(inner_reports) => reports.extend(inner_reports),
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
        if reports.is_empty() {
            bail!("Aucun profil valide parmi les archives imbriquées : {}", errors.join("; "));
        }
        return Ok(reports);
    }
    let mut profile_groups: HashMap<String, HashMap<String, Vec<u8>>> = HashMap::new();
    for (name, bytes) in &files {
        if let Some((prefix, relative)) = extract_sdprofile_root(name) {
            profile_groups
                .entry(prefix.to_string())
                .or_default()
                .insert(relative.to_string(), bytes.clone());
        }
    }
    if !profile_groups.is_empty() {
        let mut keys: Vec<_> = profile_groups.keys().cloned().collect();
        keys.sort();
        let mut reports = Vec::new();
        let mut errors = Vec::new();
        for k in keys {
            let p_files = profile_groups.remove(&k).unwrap();
            match convert(p_files) {
                Ok(report) => reports.push(report),
                Err(e) => errors.push(format!("{k}: {e}")),
            }
        }
        if reports.is_empty() {
            bail!("Aucun profil valide dans la sauvegarde : {}", errors.join("; "));
        }
        return Ok(reports);
    }
    Ok(vec![convert(files)?])
}
fn extract_sdprofile_root(path: &str) -> Option<(&str, &str)> {
    let lower = path.to_lowercase();
    let idx = lower.find(".sdprofile/")?;
    let prefix = &path[..idx + ".sdprofile".len()];
    let folder_name_start = prefix.rfind('/').map(|i| i + 1).unwrap_or(0);
    let relative = &path[folder_name_start..];
    Some((prefix, relative))
}
fn actions(value: &Value) -> Option<&serde_json::Map<String, Value>> {
    value["Actions"].as_object().or_else(|| {
        value["Controllers"]
            .as_array()?
            .iter()
            .find(|v| v["Type"] == "Keypad")?["Actions"]
            .as_object()
    })
}
fn string<'a>(v: &'a Value, names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|n| v[*n].as_str())
}
fn source_id(path: &str) -> String {
    path.rsplit('/')
        .nth(1)
        .unwrap_or_default()
        .trim_end_matches(".sdProfile")
        .to_ascii_lowercase()
}
fn convert(files: HashMap<String, Vec<u8>>) -> Result<Report> {
    let mut manifests = Vec::new();
    for (name, bytes) in &files {
        if name.ends_with("manifest.json") {
            let v: Value = serde_json::from_slice(bytes).context("Manifest JSON invalide")?;
            manifests.push((name.clone(), v));
        }
    }
    manifests.sort_by_key(|(name, _)| (name.matches('/').count(), name.clone()));
    let (root_path, root) = manifests
        .iter()
        .find(|(_, v)| v["Pages"].is_object() || actions(v).is_some() || v["UUID"].is_string())
        .cloned()
        .ok_or_else(|| anyhow!("Aucun profil pris en charge trouvé"))?;
    let mut warnings = Vec::new();
    let order = root["Pages"]["Pages"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_ascii_lowercase))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut sources = Vec::new();
    if actions(&root).is_some() {
        sources.push((source_id(&root_path), root_path.clone(), root.clone()));
    } else if root["UUID"].is_string() {
        sources.push((
            "single".into(),
            root_path.clone(),
            serde_json::json!({"Actions":{"0,0":root}}),
        ));
    }
    for (name, value) in manifests {
        if name != root_path && actions(&value).is_some() {
            sources.push((source_id(&name), name, value));
        }
    }
    ensure!(
        !sources.is_empty() && sources.len() <= 129,
        "Nombre de pages invalide (129 maximum)"
    );
    let unresolved_order =
        !order.is_empty() && !sources.iter().any(|(id, _, _)| order.contains(id));
    if unresolved_order {
        warnings.push("Les identifiants de pages ne correspondent pas aux dossiers de l’archive : chaque contenu est importé comme page indépendante. Vérifie leur ordre dans DeckForge.".into());
        sources.sort_by_key(|(_, path, v)| {
            (
                v["Name"].as_str().unwrap_or_default().is_empty(),
                v["Name"].as_str().unwrap_or_default().to_string(),
                path.clone(),
            )
        });
    } else if !order.is_empty() {
        sources.sort_by_key(|(id, _, _)| order.iter().position(|v| v == id).unwrap_or(usize::MAX));
    }
    let prefix = format!(
        "elgato-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    let ids: HashMap<_, _> = sources
        .iter()
        .enumerate()
        .map(|(i, (id, _, _))| {
            (
                id.clone(),
                if i == 0 {
                    String::new()
                } else {
                    format!("{prefix}-{i}")
                },
            )
        })
        .collect();
    let mut profile = Profile::new(
        root["Name"]
            .as_str()
            .unwrap_or("Import Stream Deck")
            .chars()
            .take(120)
            .collect(),
    );
    if profile.name.trim().is_empty() {
        profile.name = "Import Stream Deck".into();
    }
    for (i, (id, _, v)) in sources.iter().enumerate().skip(1) {
        let folder =
            !unresolved_order && (!order.is_empty() && !order.contains(id) || order.is_empty());
        profile.pages.push(Page::new(
            ids[id].clone(),
            v["Name"]
                .as_str()
                .filter(|n| !n.trim().is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    format!("{} {}", if folder { "Dossier" } else { "Page" }, i + 1)
                }),
            folder,
        ));
    }
    let mut converted: usize = 0;
    let mut icons = vec![];
    for (source, path, value) in &sources {
        let page_id = &ids[source];
        let keys = profile.keys_at_mut(page_id).unwrap();
        for (position, action) in actions(value).unwrap() {
            let coords = position
                .split(',')
                .map(str::parse::<usize>)
                .collect::<std::result::Result<Vec<_>, _>>();
            let index = match coords {
                Ok(c) if c.len() == 2 && c[0] < 5 && c[1] < 3 => c[1] * 5 + c[0],
                _ => {
                    warnings.push(format!(
                        "Position {position} hors de la grille D6 5×3 : ignorée."
                    ));
                    continue;
                }
            };
            let state = action["States"]
                .as_array()
                .and_then(|s| s.get(action["State"].as_u64().unwrap_or(0) as usize))
                .unwrap_or(&Value::Null);
            let title = string(state, &["Title"])
                .filter(|s| !s.is_empty())
                .or_else(|| action["Name"].as_str())
                .unwrap_or("Action importée")
                .chars()
                .take(128)
                .collect::<String>();
            let mut key = Key {
                label: title.clone(),
                color: [103, 143, 219],
                ..Key::default()
            };
            match convert_action(action, &ids, 0) {
                Ok(steps) => {
                    key.steps = steps;
                    converted += 1;
                }
                Err(e) => {
                    key.color = [221, 157, 65];
                    warnings.push(format!(
                        "{} · touche {} · {} : {}",
                        profile_name(value),
                        index + 1,
                        title,
                        e
                    ));
                }
            }
            if let Some(image) = state["Image"].as_str().filter(|s| !s.is_empty()) {
                if safe(image) {
                    let parent = path.rsplit_once('/').map(|p| p.0).unwrap_or("");
                    let candidates = [
                        format!("{parent}/{image}"),
                        format!("{parent}/{position}/CustomImages/{image}"),
                        image.to_string(),
                    ];
                    if let Some(bytes) = candidates.iter().find_map(|p| files.get(p)) {
                        match normalized_icon(bytes) {
                            Ok(png) => icons.push((page_id.clone(), index, png)),
                            Err(_) => warnings.push(format!(
                                "Icône de {title} ignorée : format ou taille invalide."
                            )),
                        }
                    } else {
                        warnings.push(format!(
                            "Icône de {title} absente de l’archive ; icône DeckForge utilisée."
                        ));
                    }
                } else {
                    warnings.push(format!("Chemin d’icône de {title} refusé."));
                }
            }
            keys[index] = key;
        }
    }
    // Invalid or partial actions stay empty; navigation must refer to a page of the correct type.
    let mut check = Config::default();
    check.profiles = vec![profile.clone()];
    check.active = 0;
    check.reset_navigation();
    for id in std::iter::once(String::new())
        .chain(profile.pages.iter().map(|p| p.id.clone()))
        .collect::<Vec<_>>()
    {
        for index in 0..15 {
            let key = &profile.keys_at(&id).unwrap()[index];
            if !key.steps.is_empty()
                && crate::model::validate_key_in_profile(key, &check, &profile).is_err()
            {
                warnings.push(format!(
                    "{} : configuration non convertible, laissée vide.",
                    key.label
                ));
                profile.keys_at_mut(&id).unwrap()[index].steps.clear();
                converted = converted.saturating_sub(1);
            }
        }
    }
    check.profiles[0] = profile.clone();
    check.validate()?;
    ensure!(converted>0,"Aucune action compatible dans ce fichier. Plugins, séquences partielles et raccourcis macOS ne sont pas exécutables directement.");
    Ok(Report {
        profile,
        warnings,
        converted,
        icons,
    })
}
fn profile_name(v: &Value) -> &str {
    v["Name"].as_str().unwrap_or("Page")
}
fn normalized_icon(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode()?.thumbnail(256, 256);
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)?;
    Ok(out.into_inner())
}
fn convert_action(a: &Value, ids: &HashMap<String, String>, depth: usize) -> Result<Vec<Step>> {
    ensure!(depth < 8, "Multi-action trop imbriquée");
    let uuid = a["UUID"]
        .as_str()
        .ok_or_else(|| anyhow!("Identifiant d’action absent"))?;
    let s = &a["Settings"];
    let steps = match uuid {
        "com.elgato.streamdeck.system.hotkey" => {
            let hotkeys = s["Hotkeys"]
                .as_array()
                .ok_or_else(|| anyhow!("Raccourci absent"))?;
            let active = hotkeys
                .iter()
                .filter(|v| v["VKeyCode"].as_i64().is_some_and(|n| n >= 0))
                .collect::<Vec<_>>();
            ensure!(
                active.len() == 1,
                "Raccourci macOS ou combinaison multiple non convertible"
            );
            let k = active[0];
            let vk = u16::try_from(k["VKeyCode"].as_u64().unwrap())
                .map_err(|_| anyhow!("Code de touche Windows hors limites : action non convertie"))?;
            if let Some(qt) = k["QTKeyCode"].as_u64().and_then(qt_vk) {
                ensure!(
                    qt == vk,
                    "Codes de touche Windows/Qt incohérents : à reconfigurer"
                );
            }
            let mut names = vec![];
            for (flag, name) in [
                ("KeyCtrl", "CTRL"),
                ("KeyOption", "ALT"),
                ("KeyShift", "SHIFT"),
                ("KeyCmd", "WIN"),
            ] {
                if k[flag].as_bool().unwrap_or(false) {
                    names.push(name.to_string());
                }
            }
            names.push(
                vk_name(vk).ok_or_else(|| anyhow!("Touche Windows {vk} non prise en charge"))?,
            );
            vec![Step::Hotkey {
                keys: names.join("+"),
            }]
        }
        "com.elgato.streamdeck.system.website" => vec![Step::Url {
            url: string(s, &["path", "url"])
                .ok_or_else(|| anyhow!("URL absente"))?
                .into(),
        }],
        "com.elgato.streamdeck.system.text" => {
            let mut steps = vec![Step::Text {
                text: string(s, &["text", "Text", "pastedText"])
                    .ok_or_else(|| anyhow!("Texte absent"))?
                    .into(),
            }];
            if s["isSendingEnter"].as_bool().unwrap_or(false)
                || s["sendEnter"].as_bool().unwrap_or(false)
            {
                steps.push(Step::Hotkey {
                    keys: "ENTER".into(),
                });
            }
            steps
        }
        "com.elgato.streamdeck.system.open" => {
            let path = string(s, &["path"])
                .ok_or_else(|| anyhow!("Application absente"))?
                .trim_matches('"');
            ensure!(
                path.to_lowercase().ends_with(".exe") && !path.contains('"'),
                "Seul un chemin .exe sans arguments incorporés est convertible"
            );
            vec![Step::Launch {
                program: path.into(),
                args: vec![],
                focus_if_running: false,
            }]
        }
        "com.elgato.streamdeck.profile.openchild" => {
            let id = s["ProfileUUID"]
                .as_str()
                .ok_or_else(|| anyhow!("Dossier cible absent"))?;
            vec![Step::Folder {
                id: ids
                    .get(&id.to_ascii_lowercase())
                    .ok_or_else(|| anyhow!("Dossier cible absent du fichier"))?
                    .clone(),
            }]
        }
        "com.elgato.streamdeck.profile.backtoparent" => vec![Step::Back],
        "com.elgato.streamdeck.page.next" => vec![Step::PageNext],
        "com.elgato.streamdeck.page.previous" | "com.elgato.streamdeck.page.prev" => {
            vec![Step::PagePrevious]
        }
        "com.elgato.streamdeck.multiactions" | "com.elgato.streamdeck.multi.actions" => {
            let nested = a["Actions"]
                .as_array()
                .or_else(|| s["Actions"].as_array())
                .ok_or_else(|| anyhow!("Multi-action inconnue"))?;
            let mut result = vec![];
            for action in nested {
                result.extend(convert_action(action, ids, depth + 1)?);
                ensure!(result.len() <= 256, "Plus de 256 étapes");
            }
            ensure!(!result.is_empty(), "Multi-action vide");
            result
        }
        "com.elgato.streamdeck.multiactions.delay" | "com.elgato.streamdeck.multi.delay" => {
            vec![Step::Delay {
                ms: s["delay"]
                    .as_u64()
                    .or_else(|| s["Delay"].as_u64())
                    .ok_or_else(|| anyhow!("Délai absent"))?,
            }]
        }
        _ => bail!("Plugin ou action non pris en charge ({uuid})"),
    };
    Ok(steps)
}
fn vk_name(vk: u16) -> Option<String> {
    Some(match vk {
        65..=90 | 48..=57 => (vk as u8 as char).to_string(),
        112..=135 => format!("F{}", vk - 111),
        8 => "BACKSPACE".into(),
        9 => "TAB".into(),
        13 => "ENTER".into(),
        27 => "ESC".into(),
        32 => "SPACE".into(),
        33 => "PAGEUP".into(),
        34 => "PAGEDOWN".into(),
        35 => "END".into(),
        36 => "HOME".into(),
        37 => "LEFT".into(),
        38 => "UP".into(),
        39 => "RIGHT".into(),
        40 => "DOWN".into(),
        45 => "INSERT".into(),
        46 => "DELETE".into(),
        _ => return None,
    })
}
fn qt_vk(qt: u64) -> Option<u16> {
    Some(match qt {
        48..=57 | 65..=90 | 32 => qt as u16,
        16777216 => 27,
        16777217 => 9,
        16777219 => 8,
        16777220 | 16777221 => 13,
        16777222 => 45,
        16777223 => 46,
        16777232 => 36,
        16777233 => 35,
        16777234 => 37,
        16777235 => 38,
        16777236 => 39,
        16777237 => 40,
        16777238 => 33,
        16777239 => 34,
        16777264..=16777287 => (qt - 16777264 + 112) as u16,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    fn hotkey() -> Value {
        json!({"UUID":"com.elgato.streamdeck.system.hotkey","States":[{"Title":"Save"}],"Settings":{"Hotkeys":[{"VKeyCode":83,"KeyCtrl":true},{"VKeyCode":-1}]}})
    }
    #[test]
    fn oversized_vkey_is_not_truncated() {
        let mut action = hotkey();
        action["Settings"]["Hotkeys"][0]["VKeyCode"] = json!(65619);
        let error = convert_action(&action, &HashMap::new(), 0).unwrap_err();
        assert!(error.to_string().contains("action non convertie"));
    }
    #[test]
    fn imports_legacy_and_preserves_unknown_as_empty_with_report() {
        let root = json!({"Name":"Example","Actions":{"0,0":hotkey(),"1,0":{"UUID":"third.party.plugin"}}});
        let r = convert(HashMap::from([(
            "x.sdProfile/manifest.json".into(),
            serde_json::to_vec(&root).unwrap(),
        )]))
        .unwrap();
        assert_eq!(r.converted, 1);
        assert!(matches!(&r.profile.keys[0].steps[0],Step::Hotkey{keys}if keys=="CTRL+S"));
        assert!(r.profile.keys[1].steps.is_empty());
        assert!(!r.warnings.is_empty());
    }
    #[test]
    fn imports_pages_folders_and_rejects_partial_multi_action() {
        let root = json!({"Name":"Pages","Pages":{"Pages":["A","B"]},"Version":"2.0"});
        let a = json!({"Controllers":[{"Type":"Keypad","Actions":{"0,0":{"UUID":"com.elgato.streamdeck.profile.openchild","Settings":{"ProfileUUID":"C"}},"1,0":hotkey()}}]});
        let b = json!({"Actions":{"0,0":{"UUID":"com.elgato.streamdeck.multiactions","Actions":[hotkey(),{"UUID":"unknown.plugin"}]}}});
        let c = json!({"Actions":{"0,0":{"UUID":"com.elgato.streamdeck.profile.backtoparent"}}});
        let files = [
            ("root.sdProfile/manifest.json", root),
            ("root.sdProfile/Profiles/A/manifest.json", a),
            ("root.sdProfile/Profiles/B/manifest.json", b),
            ("root.sdProfile/Profiles/C/manifest.json", c),
        ]
        .into_iter()
        .map(|(n, v)| (n.into(), serde_json::to_vec(&v).unwrap()))
        .collect();
        let r = convert(files).unwrap();
        assert_eq!(r.profile.pages.len(), 2);
        assert!(!r.profile.pages[0].folder);
        assert!(r.profile.pages[1].folder);
        assert!(r.profile.pages[0].keys[0].steps.is_empty());
        assert_eq!(r.converted, 3);
    }
    #[test]
    fn refuses_zip_path_traversal_before_any_extraction() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("bad.streamDeckProfile");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        writer
            .start_file("../manifest.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"{}").unwrap();
        writer.finish().unwrap();
        assert!(load(&path).unwrap_err().to_string().contains("Chemin"));
        assert!(!d.path().parent().unwrap().join("manifest.json").exists());
    }
    #[test]
    fn imports_streamdeckprofilesbackup_with_multiple_profiles() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("test.StreamDeckProfilesBackup");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let root1 = json!({"Name":"Streaming","Pages":{"Pages":["P1"]},"Version":"2.0"});
        let page1 = json!({"Controllers":[{"Type":"Keypad","Actions":{"0,0":hotkey()}}]});
        let root2 = json!({"Name":"Gaming","Pages":{"Pages":["P2"]},"Version":"2.0"});
        let page2 = json!({"Controllers":[{"Type":"Keypad","Actions":{"0,0":hotkey(),"1,0":hotkey()}}]});
        let opts = zip::write::SimpleFileOptions::default();
        writer.start_file("ProfilesV2/11111111-1111-1111-1111-111111111111.sdProfile/manifest.json", opts).unwrap();
        writer.write_all(&serde_json::to_vec(&root1).unwrap()).unwrap();
        writer.start_file("ProfilesV2/11111111-1111-1111-1111-111111111111.sdProfile/Profiles/P1/manifest.json", opts).unwrap();
        writer.write_all(&serde_json::to_vec(&page1).unwrap()).unwrap();
        writer.start_file("ProfilesV2/22222222-2222-2222-2222-222222222222.sdProfile/manifest.json", opts).unwrap();
        writer.write_all(&serde_json::to_vec(&root2).unwrap()).unwrap();
        writer.start_file("ProfilesV2/22222222-2222-2222-2222-222222222222.sdProfile/Profiles/P2/manifest.json", opts).unwrap();
        writer.write_all(&serde_json::to_vec(&page2).unwrap()).unwrap();
        writer.finish().unwrap();

        let reports = load_all(&path).unwrap();
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].profile.name, "Streaming");
        assert_eq!(reports[0].converted, 1);
        assert_eq!(reports[1].profile.name, "Gaming");
        assert_eq!(reports[1].converted, 2);

        let first = load(&path).unwrap();
        assert_eq!(first.profile.name, "Streaming");
    }
    #[test]
    fn imports_streamdeckprofilesbackup_with_nested_archives() {
        let d = tempfile::tempdir().unwrap();
        let p1_path = d.path().join("p1.streamDeckProfile");
        let mut w1 = zip::ZipWriter::new(std::fs::File::create(&p1_path).unwrap());
        let root1 = json!({"Name":"Music","Actions":{"0,0":hotkey()}});
        w1.start_file("manifest.json", zip::write::SimpleFileOptions::default()).unwrap();
        w1.write_all(&serde_json::to_vec(&root1).unwrap()).unwrap();
        w1.finish().unwrap();

        let p2_path = d.path().join("p2.streamDeckProfile");
        let mut w2 = zip::ZipWriter::new(std::fs::File::create(&p2_path).unwrap());
        let root2 = json!({"Name":"Dev","Actions":{"0,0":hotkey(),"1,0":hotkey()}});
        w2.start_file("manifest.json", zip::write::SimpleFileOptions::default()).unwrap();
        w2.write_all(&serde_json::to_vec(&root2).unwrap()).unwrap();
        w2.finish().unwrap();

        let backup_path = d.path().join("all.StreamDeckProfilesBackup");
        let mut wb = zip::ZipWriter::new(std::fs::File::create(&backup_path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        wb.start_file("music.streamDeckProfile", opts).unwrap();
        wb.write_all(&std::fs::read(&p1_path).unwrap()).unwrap();
        wb.start_file("dev.streamDeckProfile", opts).unwrap();
        wb.write_all(&std::fs::read(&p2_path).unwrap()).unwrap();
        wb.finish().unwrap();

        let reports = load_all(&backup_path).unwrap();
        assert_eq!(reports.len(), 2);
        assert!(reports.iter().any(|r| r.profile.name == "Music" && r.converted == 1));
        assert!(reports.iter().any(|r| r.profile.name == "Dev" && r.converted == 2));
    }
}
