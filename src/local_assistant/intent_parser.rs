/// Normalisation destinée à la reconnaissance, jamais aux valeurs des paramètres.
pub(super) fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'œ' => 'o',
            c if c.is_alphanumeric() || c == '+' || c == '%' => c,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn clauses(text: &str) -> Vec<String> {
    // Les séparateurs ne s'appliquent ni à un nombre décimal, ni à un nom cité.
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (i, &(at, ch)) in chars.iter().enumerate() {
        if matches!(ch, '"' | '«' | '»') {
            quoted = ch != '»' && !quoted;
        }
        if quoted {
            continue;
        }
        let comma = ch == ','
            && !matches!((i.checked_sub(1).and_then(|j| chars.get(j)), chars.get(i + 1)), (Some((_, a)), Some((_, b))) if a.is_ascii_digit() && b.is_ascii_digit());
        let word = [" puis ", " ensuite ", " et ", " après ", " apres ", " then ", " and then ", " and "]
            .iter()
            .find(|w| text[at..].to_lowercase().starts_with(*w));
        if comma || word.is_some() {
            let part = text[start..at].trim();
            if !part.is_empty() {
                parts.push(part.to_owned());
            }
            start = at + word.map_or(1, |w| w.len());
        }
    }
    let last = text[start..].trim();
    if !last.is_empty() {
        parts.push(last.to_owned());
    }
    parts
}

pub(super) fn quoted_or_after(text: &str, markers: &[&str]) -> Option<String> {
    for (open, close) in [('«', '»'), ('"', '"')] {
        if let Some((_, rest)) = text.split_once(open) {
            if let Some((name, _)) = rest.split_once(close) {
                return Some(name.trim().to_owned());
            }
        }
    }
    let lower = text.to_lowercase();
    for marker in markers {
        if let Some(pos) = lower.find(marker) {
            return Some(
                text[pos + marker.len()..]
                    .trim()
                    .trim_matches(['.', '!', '?'])
                    .to_owned(),
            );
        }
    }
    None
}

pub(super) fn number(text: &str) -> Option<u32> {
    normalize(text)
        .split_whitespace()
        .find_map(|w| w.parse().ok())
}

pub(super) fn percent(text: &str) -> Option<u8> {
    let n = normalize(text);
    let words: Vec<_> = n.split_whitespace().collect();
    words
        .windows(2)
        .find_map(|p| (p[1] == "%").then(|| p[0].parse::<u8>().ok()).flatten())
}

pub(super) fn duration(text: &str) -> Option<u64> {
    let mut lower = text.to_lowercase().replace(',', ".").replace('-', " ");
    for phrase in ["une demi seconde", "demi seconde", "half a second", "half second"] {
        lower = lower.replace(phrase, "500 ms");
    }
    let number = |w: &str| -> Option<&'static str> {
        Some(match w {
            "un" | "une" | "a" | "an" | "one" => "1",
            "deux" | "two" => "2",
            "trois" | "three" => "3",
            "quatre" | "four" => "4",
            "cinq" | "five" => "5",
            "dix" | "ten" => "10",
            "quinze" | "fifteen" => "15",
            "trente" | "thirty" => "30",
            _ => return None,
        })
    };
    let words: Vec<_> = lower.split_whitespace().map(|w| number(w).unwrap_or(w)).collect();
    for pair in words.windows(2) {
        let Ok(value) = pair[0].parse::<f64>() else {
            continue;
        };
        let factor = match pair[1].trim_matches(['.', '!', '?']) {
            "ms" | "milliseconde" | "millisecondes" | "millisecond" | "milliseconds" => 1.0,
            "s" | "sec" | "seconde" | "secondes" | "second" | "seconds" => 1000.0,
            "min" | "minute" | "minutes" | "mn" => 60_000.0,
            _ => continue,
        };
        let millis = value * factor;
        if millis.is_finite() && (0.0..=3_600_000.0).contains(&millis) && millis.fract() == 0.0 {
            return Some(millis as u64);
        }
    }
    None
}

pub(super) fn hotkey(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let raw = trimmed
        .replace(' ', "+")
        .to_uppercase()
        .replace("CONTROL", "CTRL")
        .replace("WINDOWS", "WIN");
    let parts: Vec<_> = raw.split('+').collect();
    if parts.len() < 2
        || !parts
            .iter()
            .any(|p| ["CTRL", "ALT", "SHIFT", "WIN"].contains(p))
    {
        return None;
    }
    crate::model::parse_hotkey(&raw).ok().map(|_| raw)
}
