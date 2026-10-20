//! Demandes structurées comprises sans IA : « si X (n')est (pas) ouvert … sinon … » et « N fois … ».
//! Les applications viennent des connaissances (`ia-locale.json`) : aucun programme n'est inventé.
use super::{has, parser};
use crate::local_llm::{Knowledge, KnownApp};
use crate::model::Step;

/// La demande commence par une condition ou contient une alternative.
pub(super) fn is_conditional(n: &str) -> bool {
    n.starts_with("si ") || n.starts_with("if ") || n.starts_with("quand ") || has(n, &["sinon", "otherwise", "else", "seulement si", "only if"])
        || ([" s il est", " s il n est", " si il est", " si il n est", " s il l est", " et si "].iter().any(|m| n.contains(m))
            && has(n, &["ouvert", "ouverte", "lance", "tourne", "open", "running"]))
}

/// La demande parle de répétition.
pub(super) fn is_repetition(n: &str) -> bool {
    has(n, &["fois", "times", "repete", "repeter", "repeat"])
}

const NEGATIVE: &[&str] = &[
    "n est pas encore ouvert", "n est pas deja ouvert", "n est pas ouvert", "n est pas ouverte", "n est pas lance", "n est pas lancee",
    "n est pas demarre", "n est pas en cours", "n est pas actif", "ne tourne pas", "est ferme", "est fermee", "pas ouvert", "pas lance",
    "is not open", "isn t open", "is not running", "isn t running", "is not launched", "isn t launched", "is not started", "is closed", "not running", "not open",
];
const POSITIVE: &[&str] = &[
    "est deja ouvert", "est deja ouverte", "est ouvert", "est ouverte", "est deja lance", "est lance", "est lancee", "est demarre",
    "est en cours", "est actif", "tourne", "is already open", "is open", "is running", "is launched", "is started", "runs",
];
const FILLERS: &[&str] = &[
    "alors", "then", "s il est deja ouvert", "s il est ouvert", "s il l est", "s il tourne", "si il est deja ouvert", "si il est ouvert",
    "si c est le cas", "if it is already open", "if it s already open", "if it is open", "if it s open", "if so", "dans ce cas",
    "il faut", "juste", "simplement", "seulement", "just", "only", "please", "stp", "s il te plait",
];

fn strip_fillers(text: &str) -> String {
    let mut t = text.trim().to_owned();
    loop {
        let before = t.clone();
        for f in FILLERS {
            if let Some(rest) = t.strip_prefix(f) {
                if rest.is_empty() || rest.starts_with(' ') {
                    t = rest.trim().to_owned();
                }
            }
        }
        if t == before {
            return t;
        }
    }
}

fn focus(app: &KnownApp) -> Step {
    Step::WindowOp { target: "exe".into(), value: app.exe.clone(), op: "activate".into(), x: 0, y: 0, w: 800, h: 600, monitor: 2 }
}

fn launch(app: &KnownApp) -> Step {
    Step::Launch { program: app.exe.clone(), args: vec![], focus_if_running: false }
}

const OPEN: &[&str] = &["ouvre", "ouvrir", "ouvre le", "ouvre la", "lance", "lancer", "demarre", "demarrer", "execute", "open", "start", "launch", "run"];

/// Branche d'une condition sur `app`. `rules` traduit une branche ordinaire. `None` : non compris.
fn branch(text: &str, app: &KnownApp, rules: &dyn Fn(&str) -> Option<Vec<Step>>) -> Option<Vec<Step>> {
    let n = strip_fillers(text);
    if n.is_empty() || has(&n, &["ne fais rien", "rien", "do nothing", "nothing"]) {
        return Some(vec![]);
    }
    let shortcut = |keys: &str| Some(vec![focus(app), Step::Hotkey { keys: keys.into() }]);
    if has(&n, &["rouvre l onglet", "rouvrir l onglet", "onglet ferme", "reopen", "restore the tab"]) {
        return shortcut("CTRL+SHIFT+T");
    }
    if has(&n, &["onglet precedent", "previous tab"]) {
        return shortcut("CTRL+SHIFT+TAB");
    }
    if has(&n, &["onglet suivant", "next tab"]) {
        return shortcut("CTRL+TAB");
    }
    if has(&n, &["plein ecran", "full screen", "fullscreen"]) {
        return shortcut("F11");
    }
    if has(&n, &["nouvel onglet", "nouveau onglet", "un onglet", "new tab", "a tab"]) {
        return shortcut("CTRL+T");
    }
    if has(&n, &["nouvelle fenetre", "new window"]) {
        return shortcut("CTRL+N");
    }
    if has(&n, &["navigation privee", "fenetre privee", "private window", "incognito"]) {
        return shortcut("CTRL+SHIFT+N");
    }
    if has(&n, &["ferme l onglet", "ferme cet onglet", "close the tab", "close tab"]) {
        return shortcut("CTRL+W");
    }
    if has(&n, &["recharge", "actualise", "rafraichis", "reload", "refresh"]) {
        return shortcut("F5");
    }
    if has(&n, &["premier plan", "en avant", "affiche", "montre", "focus", "bring", "switch to", "bascule", "active", "front"]) {
        return Some(vec![focus(app)]);
    }
    if has(&n, OPEN) {
        // « ouvre-le », « lance Brave », « start it » : le programme de la condition.
        let rest = n.split_whitespace().filter(|w| !OPEN.contains(w) && !["le", "la", "l", "it", "the", "app", "application", "programme"].contains(w)).count();
        let same_app = app.noms.iter().any(|name| n.contains(&parser::normalize(name)));
        if rest == 0 || same_app {
            return Some(vec![launch(app)]);
        }
    }
    rules(&n).filter(|steps| !steps.is_empty())
}

/// « si X (n')est (pas) ouvert, A, sinon B » → Si l'application X tourne : …, sinon : …
pub(super) fn condition(text: &str, knowledge: &Knowledge, rules: &dyn Fn(&str) -> Option<Vec<Step>>) -> Option<(Step, String)> {
    let n = parser::normalize(text);
    let Some(body) = ["si ", "if ", "quand "].iter().find_map(|p| n.strip_prefix(p)) else {
        return postfix(&n, knowledge, rules);
    };
    let (head, otherwise) = match [" sinon ", " otherwise ", " else ", " autrement "].iter().filter_map(|w| body.find(w).map(|i| (i, w.len()))).min() {
        Some((i, len)) => (&body[..i], &body[i + len..]),
        None => (body, ""),
    };
    let found = NEGATIVE.iter().map(|p| (p, true)).chain(POSITIVE.iter().map(|p| (p, false)))
        .filter_map(|(p, neg)| head.find(&format!(" {p}")).map(|i| (i, p.len() + 1, neg)))
        .min_by_key(|(i, len, _)| (*i, usize::MAX - len))?;
    let (index, len, negative) = found;
    let subject = &head[..index];
    let then_text = &head[index + len..];
    let app = knowledge.find_app(subject)?;
    let then_branch = branch(then_text, app, rules)?;
    let else_branch = branch(otherwise, app, rules)?;
    let (then_steps, else_steps) = if negative { (else_branch, then_branch) } else { (then_branch, else_branch) };
    if then_steps.is_empty() && else_steps.is_empty() {
        return None;
    }
    let name = app.noms.first().cloned().unwrap_or_else(|| app.exe.clone());
    Some((
        Step::IfCondition { condition: crate::conditions::Condition::AppRunning { exe: app.exe.clone() }, then_steps, else_steps },
        format!("Condition : « {name} » ({}) est lancé ? « Sinon » s'applique quand il ne l'est pas.", app.exe),
    ))
}

/// Condition écrite après l'action : « ouvre Brave s'il n'est pas ouvert, et s'il l'est ouvre un
/// nouvel onglet ». Réécrite sous la forme « si Brave … , A sinon B », sans application inventée.
fn postfix(n: &str, knowledge: &Knowledge, rules: &dyn Fn(&str) -> Option<Vec<Step>>) -> Option<(Step, String)> {
    let (at, len) = [" s il ", " si il ", " si elle ", " s ils ", " si "].iter()
        .filter_map(|m| n.find(m).map(|i| (i, m.len())))
        .filter(|(i, len)| !n[i + len..].starts_with("te plait") && !n[i + len..].starts_with("vous plait"))
        .min()?;
    let action = n[..at].trim().trim_end_matches(',').trim();
    let tail = &n[at + len..];
    let (cond, other) = match [" et si ", " et s il ", " mais si ", " mais s il ", " sinon ", " s il ", " si il ", " si "].iter()
        .filter_map(|m| tail.find(m).map(|i| (i, m.len()))).min() {
        Some((i, l)) => (tail[..i].trim().trim_end_matches(','), tail[i + l..].trim()),
        None => (tail.trim(), ""),
    };
    let app = knowledge.find_app(action).or_else(|| knowledge.find_app(cond))?;
    let negative = cond.split_whitespace().any(|w| w == "pas" || w == "not") || has(cond, &["ferme", "closed", "isn t"]);
    if !NEGATIVE.iter().chain(POSITIVE).any(|p| format!(" {cond}").contains(&format!(" {p}")) || cond.ends_with(p.rsplit(' ').next().unwrap_or(p))) {
        return None;
    }
    // Retire la deuxième condition (« il est déjà ouvert ») pour garder l'action.
    const COND_WORDS: &[&str] = &["il", "elle", "l", "est", "deja", "ouvert", "ouverte", "lance", "lancee", "demarre", "tourne", "actif", "en", "cours", "c", "le", "cas", "it", "is", "already", "open", "running", "alors", "then"];
    let words: Vec<&str> = other.split_whitespace().collect();
    let skip = words.iter().take_while(|w| COND_WORDS.contains(w)).count();
    if words[..skip].iter().any(|w| *w == "pas" || *w == "not") {
        return None;
    }
    let other_action = words[skip..].join(" ");
    let name = parser::normalize(app.noms.first().map(String::as_str).unwrap_or(&app.exe));
    let state = if negative { "n est pas ouvert" } else { "est ouvert" };
    condition(&format!("si {name} {state} {action} sinon {other_action}"), knowledge, rules)
}

fn number(word: &str) -> Option<u32> {
    if let Ok(n) = word.parse::<u32>() {
        return (1..=50).contains(&n).then_some(n);
    }
    Some(match word {
        "deux" | "two" | "twice" => 2,
        "trois" | "three" => 3,
        "quatre" | "four" => 4,
        "cinq" | "five" => 5,
        "six" => 6,
        "sept" | "seven" => 7,
        "huit" | "eight" => 8,
        "neuf" | "nine" => 9,
        "dix" | "ten" => 10,
        _ => return None,
    })
}

/// « appuie 5 fois sur entrée », « répète 3 fois : … », « press enter 4 times ».
pub(super) fn repetition(text: &str, rules: &dyn Fn(&str) -> Option<Vec<Step>>) -> Option<Step> {
    let n = parser::normalize(text);
    let words: Vec<&str> = n.split_whitespace().collect();
    let at = words.iter().position(|w| *w == "fois" || *w == "times")?;
    let count = number(words.get(at.checked_sub(1)?)?)?;
    let body: Vec<&str> = words.iter().enumerate()
        .filter(|(i, w)| *i != at && *i + 1 != at && !["repete", "repeter", "repeat", "chaque", "each"].contains(w))
        .map(|(_, w)| *w)
        .collect();
    let steps = rules(&body.join(" "))?;
    (!steps.is_empty() && steps.len() <= 10).then_some(Step::Repeat { count, steps })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Config;

    fn rules(text: &str) -> Option<Vec<Step>> {
        let p = super::super::propose(text, &Config::default());
        matches!(p.status, super::super::Status::Reconnue).then_some(p.steps)
    }

    #[test]
    fn brave_open_or_new_tab_is_one_condition() {
        let k = Knowledge::builtin();
        let (step, note) = condition("si Brave n'est pas ouvert , ouvrir Brave sinon s'il est deja ouvert ouvre juste un nouvel onglet", &k, &rules).expect("compris");
        assert!(note.contains("brave.exe"));
        match step {
            Step::IfCondition { condition: crate::conditions::Condition::AppRunning { exe }, then_steps, else_steps } => {
                assert_eq!(exe, "brave.exe");
                assert!(matches!(&then_steps[..], [Step::WindowOp { target, value, op, .. }, Step::Hotkey { keys }] if target == "exe" && value == "brave.exe" && op == "activate" && keys == "CTRL+T"), "{then_steps:?}");
                assert!(matches!(&else_steps[..], [Step::Launch { program, .. }] if program == "brave.exe"), "{else_steps:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn condition_written_after_the_action_is_understood() {
        let k = Knowledge::builtin();
        for text in [
            "ouvre le navigateur brave s'il est pas ouvert et si il deja ouvert ouvre un nouvel onglet",
            "lance brave s'il n'est pas ouvert sinon ouvre un nouvel onglet",
            "ouvre brave si brave n'est pas ouvert, s'il est déjà ouvert ouvre un nouvel onglet",
        ] {
            let (step, _) = condition(text, &k, &rules).unwrap_or_else(|| panic!("non compris : {text}"));
            match step {
                Step::IfCondition { condition: crate::conditions::Condition::AppRunning { exe }, then_steps, else_steps } => {
                    assert_eq!(exe, "brave.exe");
                    assert!(matches!(&then_steps[..], [Step::WindowOp { op, .. }, Step::Hotkey { keys }] if op == "activate" && keys == "CTRL+T"), "{text}: {then_steps:?}");
                    assert!(matches!(&else_steps[..], [Step::Launch { program, .. }] if program == "brave.exe"), "{text}: {else_steps:?}");
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(super::super::propose("ouvre le navigateur brave s'il est pas ouvert et si il deja ouvert ouvre un nouvel onglet", &Config::default()).steps.len() == 1);
        assert!(condition("ouvre brave s'il te plait", &k, &rules).is_none());
    }

    #[test]
    fn browser_tab_shortcuts_in_conditions() {
        let k = Knowledge::builtin();
        let keys = |t: &str| match condition(t, &k, &rules) {
            Some((Step::IfCondition { then_steps, .. }, _)) => then_steps.iter().find_map(|s| if let Step::Hotkey { keys } = s { Some(keys.clone()) } else { None }),
            _ => None,
        };
        assert_eq!(keys("si brave est ouvert rouvre l'onglet ferme").as_deref(), Some("CTRL+SHIFT+T"));
        assert_eq!(keys("si brave est ouvert passe a l'onglet suivant").as_deref(), Some("CTRL+TAB"));
        assert_eq!(keys("si brave est ouvert passe a l'onglet precedent").as_deref(), Some("CTRL+SHIFT+TAB"));
        assert_eq!(keys("si brave est ouvert mets le en plein ecran").as_deref(), Some("F11"));
    }

    #[test]
    fn other_conditions_and_repetitions() {
        let k = Knowledge::builtin();
        let cond = |t: &str| condition(t, &k, &rules).map(|(s, _)| s);
        assert!(matches!(cond("si spotify tourne mets pause sinon lance spotify"), Some(Step::IfCondition { then_steps, else_steps, .. })
            if matches!(&then_steps[..], [Step::Media { command }] if command == "play_pause") && matches!(&else_steps[..], [Step::Launch { program, .. }] if program == "Spotify.exe")));
        assert!(matches!(cond("if discord is open bring it to the front otherwise start it"), Some(Step::IfCondition { then_steps, else_steps, .. })
            if matches!(&then_steps[..], [Step::WindowOp { op, .. }] if op == "activate") && else_steps.len() == 1));
        assert!(matches!(cond("si vlc est lancé mets la musique en pause"), Some(Step::IfCondition { then_steps, else_steps, .. }) if then_steps.len() == 1 && else_steps.is_empty()));
        assert!(cond("si monappinconnue est ouverte ferme la").is_none(), "application inconnue : pas de programme inventé");
        let rep = |t: &str| repetition(t, &rules);
        assert!(matches!(rep("appuie 5 fois sur entrée"), Some(Step::Repeat { count: 5, steps }) if matches!(&steps[..], [Step::Hotkey { keys }] if keys == "ENTER")));
        assert!(matches!(rep("press the down arrow three times"), Some(Step::Repeat { count: 3, steps }) if matches!(&steps[..], [Step::Hotkey { keys }] if keys == "DOWN")));
    }
}
