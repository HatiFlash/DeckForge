//! Traductions de l'interface.
//!
//! Le texte français écrit dans le code est la clé : `tr("Réglages")`. Chaque langue est un fichier
//! `res/i18n/<code>.json` (`{"language": "English", "strings": {"Réglages": "Settings"}}`) intégré
//! automatiquement à la compilation. Une chaîne absente retombe sur l'anglais, puis sur le français.
//! Variables : `trf("Version {version}", &[("version", &v)])` ; les `{noms}` doivent être conservés.
use serde::Deserialize;
use std::{collections::HashMap, fmt::Display, sync::{atomic::{AtomicUsize, Ordering}, OnceLock}};

include!(concat!(env!("OUT_DIR"), "/i18n_files.rs"));

pub const SOURCE: &str = "fr";
const FALLBACK: &str = "en";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File { language: String, strings: HashMap<String, String> }

pub struct Language { pub code: &'static str, pub name: String, strings: HashMap<&'static str, &'static str> }

fn languages_all() -> &'static [Language] {
    static ALL: OnceLock<Vec<Language>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut all = vec![Language { code: SOURCE, name: "Français".into(), strings: HashMap::new() }];
        for (code, text) in FILES {
            // A broken file must never prevent NovyDeck from starting: the language is skipped
            // (the `translation_files_are_valid` test catches it before a release).
            let Ok(file) = serde_json::from_str::<File>(text) else { continue };
            let strings = file.strings.into_iter()
                .filter(|(_, v)| !v.trim().is_empty())
                .map(|(k, v)| (&*Box::leak(k.into_boxed_str()), &*Box::leak(v.into_boxed_str())))
                .collect();
            all.push(Language { code, name: file.language, strings });
        }
        all
    })
}

/// Langues proposées : (code, nom dans sa propre langue), français en premier.
pub fn languages() -> Vec<(&'static str, &'static str)> {
    languages_all().iter().map(|l| (l.code, l.name.as_str())).collect()
}

fn index_of(code: &str) -> Option<usize> { languages_all().iter().position(|l| l.code == code) }

static GLOBAL: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
thread_local! {
    // Tests run in parallel: a language chosen by one test must not leak into another.
    static OVERRIDE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

fn current_index() -> usize {
    #[cfg(test)]
    if let Some(i) = OVERRIDE.with(|o| o.get()) { return i; }
    GLOBAL.load(Ordering::Relaxed)
}

/// Code de la langue active.
pub fn current() -> &'static str { languages_all()[current_index().min(languages_all().len() - 1)].code }

/// Active une langue (code inconnu = français). Retourne le code réellement appliqué.
pub fn set_language(code: &str) -> &'static str {
    let index = index_of(code).unwrap_or(0);
    #[cfg(test)]
    OVERRIDE.with(|o| o.set(Some(index)));
    #[cfg(not(test))]
    GLOBAL.store(index, Ordering::Relaxed);
    languages_all()[index].code
}

/// Langue à utiliser : préférence explicite, sinon celle de Windows si elle est traduite, sinon l'anglais.
pub fn resolve(preference: &str) -> &'static str {
    if let Some(i) = index_of(preference) { return languages_all()[i].code; }
    let system = system_language();
    index_of(&system).or_else(|| index_of(FALLBACK)).map(|i| languages_all()[i].code).unwrap_or(SOURCE)
}

/// Langue d'affichage de Windows réduite à son code principal (`fr-BE` -> `fr`).
pub fn system_language() -> String {
    #[cfg(windows)]
    {
        let mut buffer = [0u16; 85];
        let len = unsafe { windows_sys::Win32::Globalization::GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
        if len > 1 {
            let name = String::from_utf16_lossy(&buffer[..len as usize - 1]);
            return primary(&name);
        }
    }
    String::new()
}
fn primary(name: &str) -> String { name.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase() }

fn lookup(index: usize, key: &'static str) -> &'static str {
    let all = languages_all();
    if index == 0 { return key; }
    if let Some(v) = all.get(index).and_then(|l| l.strings.get(key)) { return v; }
    index_of(FALLBACK).and_then(|i| all[i].strings.get(key).copied()).unwrap_or(key)
}

/// Traduit un texte fixe de l'interface.
pub fn tr(fr: &'static str) -> &'static str { lookup(current_index(), fr) }

/// Traduit un texte contenant des variables `{nom}`.
pub fn trf(fr: &'static str, args: &[(&str, &dyn Display)]) -> String {
    let mut text = tr(fr).to_string();
    for (name, value) in args { text = text.replace(&format!("{{{name}}}"), &value.to_string()); }
    text
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Extrait les littéraux passés à `tr("…")` et `trf("…"` dans les sources.
    pub(crate) fn source_keys() -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            // i18n.rs only contains examples and test strings.
            if path.extension().map_or(true, |x| x != "rs") || path.file_name().is_some_and(|n| n == "i18n.rs") { continue; }
            let text = std::fs::read_to_string(&path).unwrap();
            for marker in ["tr(", "trf("] {
                let mut rest = text.as_str();
                while let Some(pos) = rest.find(marker) {
                    // Ignore identifiers that merely end in `tr` (e.g. `attr(`).
                    let before = rest[..pos].chars().next_back();
                    rest = &rest[pos + marker.len()..];
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') { continue; }
                    // Multi-line calls: `trf(\n    "texte",`.
                    let Some(literal) = rest.trim_start().strip_prefix('"') else { continue };
                    rest = literal;
                    let mut key = String::new();
                    let mut chars = rest.chars();
                    while let Some(c) = chars.next() {
                        match c {
                            '"' => break,
                            '\\' => match chars.next() {
                                Some('n') => key.push('\n'),
                                Some('t') => key.push('\t'),
                                Some(other) => key.push(other),
                                None => break,
                            },
                            c => key.push(c),
                        }
                    }
                    keys.insert(key);
                }
            }
        }
        keys.remove("");
        keys
    }

    fn placeholders(text: &str) -> BTreeSet<String> {
        let mut set = BTreeSet::new();
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            rest = &rest[start + 1..];
            if let Some(end) = rest.find('}') {
                let name = &rest[..end];
                if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { set.insert(name.to_string()); }
                rest = &rest[end + 1..];
            }
        }
        set
    }

    #[test]
    fn translation_files_are_valid() {
        assert!(!FILES.is_empty(), "res/i18n/en.json manquant");
        for (code, text) in FILES {
            let file: File = serde_json::from_str(text).unwrap_or_else(|e| panic!("res/i18n/{code}.json invalide : {e}"));
            assert!(!file.language.trim().is_empty(), "{code} : nom de langue vide");
            assert!(code.len() >= 2 && code.chars().all(|c| c.is_ascii_lowercase() || c == '-'), "code de langue invalide : {code}");
            for (key, value) in &file.strings {
                if value.trim().is_empty() { continue; }
                assert_eq!(placeholders(key), placeholders(value), "{code} : variables différentes pour « {key} »");
            }
        }
    }

    #[test]
    fn english_covers_every_interface_text() {
        let en: File = serde_json::from_str(FILES.iter().find(|(c, _)| *c == "en").unwrap().1).unwrap();
        let keys = source_keys();
        let missing: Vec<_> = keys.iter().filter(|k| en.strings.get(*k).map_or(true, |v| v.trim().is_empty())).collect();
        assert!(missing.is_empty(), "{} texte(s) sans traduction anglaise dans res/i18n/en.json : {missing:#?}", missing.len());
        let unused: Vec<_> = en.strings.keys().filter(|k| !keys.contains(*k)).collect();
        assert!(unused.is_empty(), "{} traduction(s) inutilisée(s) dans res/i18n/en.json : {unused:#?}", unused.len());
    }

    #[test]
    fn lookup_falls_back_and_formats() {
        set_language("en");
        assert_eq!(current(), "en");
        assert_eq!(tr("Texte absent de toute traduction"), "Texte absent de toute traduction");
        set_language("xx-inconnu");
        assert_eq!(current(), "fr");
        assert_eq!(trf("Version {version}", &[("version", &"1.2.3")]), "Version 1.2.3");
        assert_eq!(primary("fr-BE"), "fr");
        assert_eq!(primary("pt_BR"), "pt");
        assert_eq!(resolve("en"), "en");
        assert!(languages().iter().any(|(c, _)| *c == "fr"));
    }
}
