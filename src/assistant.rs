use crate::model::{Config, Key, Profile, Step};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};
#[derive(Clone)]
pub enum Change {
    Config(Config),
    Run(Key),
}
pub struct Proposal {
    pub id: String,
    pub title: String,
    pub details: String,
    pub change: Change,
    base: String,
    created: Instant,
}
#[derive(Default)]
pub struct Proposals {
    pub pending: VecDeque<Proposal>,
    history: BTreeMap<String, String>,
    next: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    profile: String,
    #[serde(default)]
    page: String,
    key: usize,
    label: String,
    #[serde(default)]
    color: Option<[u8; 3]>,
    #[serde(default)]
    background: Option<bool>,
    #[serde(default)]
    repeat: Option<u32>,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub profile: String,
    #[serde(default)]
    pub page: String,
    pub key: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    id: String,
}
pub fn position(c: &Config, a: &Value) -> Result<(usize, String, usize), String> {
    let p: Position = serde_json::from_value(a.clone()).map_err(|e| e.to_string())?;
    let (profile, key) = locate(c, &p.profile, &p.page, p.key)?;
    Ok((profile, p.page, key))
}
fn locate(c: &Config, name: &str, page: &str, key: usize) -> Result<(usize, usize), String> {
    if !(1..=15).contains(&key) {
        return Err("La touche doit être comprise entre 1 et 15".into());
    }
    let p = c
        .profiles
        .iter()
        .position(|p| p.name == name)
        .ok_or("Profil introuvable")?;
    if c.profiles[p].keys_at(page).is_none() {
        return Err("Page ou dossier introuvable".into());
    }
    Ok((p, key - 1))
}
impl Proposals {
    pub fn propose(&mut self, tool: &str, args: &Value, c: &Config) -> Result<Value, String> {
        if self.pending.len() >= 10 {
            return Err("Dix propositions attendent déjà une validation dans DeckForge".into());
        }
        let (title, details, change) = match tool {
            "deckforge_create_page" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct NewPage {
                    profile: String,
                    id: String,
                    name: String,
                    folder: bool,
                }
                let a: NewPage = serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
                if a.name.trim().is_empty()
                    || a.name.len() > 128
                    || a.id.is_empty()
                    || a.id.len() > 160
                {
                    return Err("Nom ou identifiant de page invalide".into());
                }
                let mut next = c.clone();
                let p = next
                    .profiles
                    .iter_mut()
                    .find(|p| p.name == a.profile)
                    .ok_or("Profil introuvable")?;
                if p.pages.len() >= 128 || p.pages.iter().any(|p| p.id == a.id) {
                    return Err("Identifiant déjà utilisé ou limite de pages atteinte".into());
                }
                p.pages.push(crate::model::Page::new(
                    a.id.clone(),
                    a.name.clone(),
                    a.folder,
                ));
                next.validate_structure().map_err(|e| e.to_string())?;
                (
                    format!(
                        "Créer {} « {} »",
                        if a.folder { "le dossier" } else { "la page" },
                        a.name
                    ),
                    format!("Profil : {} · identifiant : {}", a.profile, a.id),
                    Change::Config(next),
                )
            }
            "deckforge_create_profile" => {
                let a: Create = serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
                if a.name.trim().is_empty() || a.name.len() > 128 {
                    return Err("Nom de profil invalide".into());
                }
                if c.profiles
                    .iter()
                    .any(|p| p.name.to_lowercase() == a.name.to_lowercase())
                {
                    return Err("Ce profil existe déjà".into());
                }
                let mut next = c.clone();
                next.profiles.push(Profile::new(a.name.clone()));
                next.active = next.profiles.len() - 1;
                next.reset_navigation();
                next.validate_structure().map_err(|e| e.to_string())?;
                (
                    format!("Créer le profil « {} »", a.name),
                    "Un nouveau profil vide de 15 touches sera créé et sélectionné.".into(),
                    Change::Config(next),
                )
            }
            "deckforge_configure_key" => {
                let a: Binding = serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
                if a.label.len() > 128 {
                    return Err("Titre trop long".into());
                }
                let (p, k) = locate(c, &a.profile, &a.page, a.key)?;
                let mut next = c.clone();
                let old = &c.profiles[p].keys_at(&a.page).unwrap()[k];
                next.profiles[p].keys_at_mut(&a.page).unwrap()[k] = Key {
                    label: a.label.clone(),
                    color: a.color.unwrap_or(old.color),
                    background: a.background.unwrap_or(old.background),
                    image: old.image.clone(),
                    repeat: a.repeat.unwrap_or(1),
                    title_size: old.title_size,
                    steps: a.steps,
                    display_kind: None,
                };
                next.validate_structure().map_err(|e| e.to_string())?;
                crate::model::validate_key_in_profile(
                    &next.profiles[p].keys_at(&a.page).unwrap()[k],
                    &next,
                    &next.profiles[p],
                )
                .map_err(|e| e.to_string())?;
                let details = describe(&next.profiles[p].keys_at(&a.page).unwrap()[k]);
                (
                    format!(
                        "Configurer {} · touche {:02} · {}",
                        a.profile, a.key, a.label
                    ),
                    details,
                    Change::Config(next),
                )
            }
            "deckforge_request_run" => {
                let (p, page, k) = position(c, args)?;
                let key = c.profiles[p].keys_at(&page).unwrap()[k].clone();
                if key.steps.iter().any(|s| {
                    matches!(
                        s,
                        Step::Page { .. }
                            | Step::Folder { .. }
                            | Step::Back
                            | Step::PageNext
                            | Step::PagePrevious
                    )
                }) && (p != c.active || page != c.page)
                {
                    return Err(
                        "Ouvre ce profil et cette page avant de tester sa navigation".into(),
                    );
                }
                crate::model::validate_key(&key, c).map_err(|e| e.to_string())?;
                if key.steps.is_empty() {
                    return Err("Cette touche n’a aucune action".into());
                }
                let details = describe(&key);
                (
                    format!("Exécuter « {} » dans 3 secondes", key.label),
                    details,
                    Change::Run(key),
                )
            }
            _ => return Err("Cet outil ne crée pas de proposition".into()),
        };
        self.next += 1;
        let id = format!("proposal-{}", self.next);
        self.pending.push_back(Proposal {
            id: id.clone(),
            title,
            details,
            change,
            base: serde_json::to_string(c).unwrap(),
            created: Instant::now(),
        });
        Ok(
            json!({"id":id,"status":"pending","message":"Validation nécessaire dans DeckForge. Aucune modification ni exécution réelle n’a encore eu lieu."}),
        )
    }
    pub fn status(&self, args: &Value) -> Result<Value, String> {
        let a: Status = serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
        let status = if self.pending.iter().any(|p| p.id == a.id) {
            "pending"
        } else {
            self.history
                .get(&a.id)
                .map(String::as_str)
                .ok_or("Proposition inconnue")?
        };
        Ok(json!({"id":a.id,"status":status}))
    }
    pub fn decide(&mut self, accept: bool, c: &Config) -> Result<Option<Change>, String> {
        let p = self.pending.pop_front().ok_or("Aucune proposition")?;
        let result = if !accept {
            self.history.insert(p.id, "rejected".into());
            Ok(None)
        } else if p.created.elapsed() > Duration::from_secs(600)
            || p.base != serde_json::to_string(c).unwrap()
        {
            self.history.insert(p.id, "stale".into());
            Err("La configuration a changé depuis la proposition. Demande une nouvelle version à l’IA.".into())
        } else {
            self.history.insert(p.id, "approved".into());
            Ok(Some(p.change))
        };
        while self.history.len() > 100 {
            let first = self.history.keys().next().unwrap().clone();
            self.history.remove(&first);
        }
        result
    }
    pub fn reject_all(&mut self) {
        while let Some(p) = self.pending.pop_front() {
            self.history.insert(p.id, "rejected".into());
        }
    }
}
fn describe(key: &Key) -> String {
    let background = if key.background {
        format!(
            "couleur #{:02X}{:02X}{:02X}",
            key.color[0], key.color[1], key.color[2]
        )
    } else {
        "sans couleur (noir)".to_string()
    };
    let mut text = format!(
        "{}\nRépétitions : {}\nFond : {}\n\n",
        key.label, key.repeat, background
    );
    for (i, step) in key.steps.iter().enumerate() {
        let detail=match step{
        Step::Delay{ms}=>format!("Attendre {ms} ms"),Step::Hotkey{keys}=>format!("Raccourci : {keys}"),Step::Text{text}=>format!("Saisir ce texte :\n{text}"),
        Step::Launch{program,args}=>format!("Ouvrir : {program}\nArguments : {}",args.join(" | ")),Step::Url{url}=>format!("Ouvrir le site : {url}"),
        Step::WaitWindow{title,timeout_ms,focus}=>format!("Attendre une fenêtre contenant « {title} » ({timeout_ms} ms maximum)\nMettre au premier plan : {}",if *focus{"oui"}else{"non"}),
        Step::Media{command}=>format!("Média / volume : {command}"),Step::Profile{name}=>format!("Passer au profil : {name}"),Step::Page{id}=>format!("Aller à la page : {id}"),Step::Folder{id}=>format!("Ouvrir le dossier : {id}"),Step::Back=>"Retour au dossier précédent".into(),Step::PageNext=>"Page suivante".into(),Step::PagePrevious=>"Page précédente".into(), _=>serde_json::to_string_pretty(step).unwrap_or_default()};
        text.push_str(&format!("{:02}. {detail}\n\n", i + 1));
    }
    text
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mcp_background_choice_is_reviewed_and_omission_keeps_the_existing_setting() {
        let mut c = Config::default();
        let original = c.keys()[1].color;
        let mut q = Proposals::default();
        q.propose("deckforge_configure_key", &json!({"profile":"Bureau","key":2,"label":"Lecture","background":false,"steps":[{"type":"media","command":"play_pause"}]}), &c).unwrap();
        assert!(q.pending[0].details.contains("Fond : sans couleur (noir)"));
        assert!(c.keys()[1].background);
        let Some(Change::Config(next)) = q.decide(true, &c).unwrap() else {
            panic!()
        };
        c = next;
        assert!(!c.keys()[1].background);
        assert_eq!(c.keys()[1].color, original);
        q.propose("deckforge_configure_key", &json!({"profile":"Bureau","key":2,"label":"Lecture modifiée","steps":[{"type":"media","command":"play_pause"}]}), &c).unwrap();
        let Some(Change::Config(next)) = q.decide(true, &c).unwrap() else {
            panic!()
        };
        assert!(!next.keys()[1].background);
        assert_eq!(next.keys()[1].color, original);
    }
    #[test]
    fn ai_can_propose_pages_and_nested_blocks_without_applying_them() {
        let mut c = Config::default();
        let mut q = Proposals::default();
        q.propose(
            "deckforge_create_page",
            &json!({"profile":"Bureau","id":"work","name":"Travail","folder":true}),
            &c,
        )
        .unwrap();
        assert!(c.profiles[0].pages.is_empty());
        let Some(Change::Config(next)) = q.decide(true, &c).unwrap() else {
            panic!()
        };
        c = next;
        assert_eq!(c.profiles[0].pages[0].id, "work");
        q.propose("deckforge_configure_key",&json!({"profile":"Bureau","page":"work","key":2,"label":"Boucle","steps":[{"type":"repeat","count":3,"steps":[{"type":"delay","ms":250}]}]}),&c).unwrap();
        assert!(c.profiles[0].pages[0].keys[1].steps.is_empty());
        let Some(Change::Config(next)) = q.decide(true, &c).unwrap() else {
            panic!()
        };
        assert!(matches!(
            next.profiles[0].pages[0].keys[1].steps[0],
            Step::Repeat { count: 3, .. }
        ));
    }
    #[test]
    fn ai_changes_remain_drafts_until_user_approval() {
        let c = Config::default();
        let original = serde_json::to_string(&c).unwrap();
        let mut queue = Proposals::default();
        let r=queue.propose("deckforge_configure_key",&json!({"profile":"Bureau","key":2,"label":"Play","steps":[{"type":"media","command":"play_pause"}]}),&c).unwrap();
        assert_eq!(r["status"], "pending");
        assert_eq!(serde_json::to_string(&c).unwrap(), original);
        assert!(matches!(
            queue.decide(true, &c).unwrap(),
            Some(Change::Config(_))
        ));
    }
    #[test]
    fn edits_invalidate_pending_ai_proposals() {
        let mut c = Config::default();
        let mut q = Proposals::default();
        q.propose("deckforge_create_profile", &json!({"name":"OBS"}), &c)
            .unwrap();
        c.brightness = 20;
        assert!(q.decide(true, &c).is_err());
        assert_eq!(
            q.status(&json!({"id":"proposal-1"})).unwrap()["status"],
            "stale"
        );
    }
    #[test]
    fn invalid_macros_and_out_of_range_keys_never_enter_queue() {
        let c = Config::default();
        let mut q = Proposals::default();
        assert!(q
            .propose(
                "deckforge_configure_key",
                &json!({"profile":"Bureau","key":16,"label":"X","steps":[]}),
                &c
            )
            .is_err());
        assert!(q.propose("deckforge_configure_key",&json!({"profile":"Bureau","key":2,"label":"X","steps":[{"type":"hotkey","keys":"NOT_A_KEY"}]}),&c).is_err());
        assert!(q.pending.is_empty());
    }
    #[test]
    fn run_request_does_not_execute_and_can_be_rejected() {
        let c = Config::default();
        let mut q = Proposals::default();
        q.propose(
            "deckforge_request_run",
            &json!({"profile":"Bureau","key":2}),
            &c,
        )
        .unwrap();
        assert!(matches!(q.pending[0].change, Change::Run(_)));
        assert!(q.decide(false, &c).unwrap().is_none());
    }
}
