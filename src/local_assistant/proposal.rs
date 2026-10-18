use crate::model::Step;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Reconnue,
    Ambigue {
        question: String,
        choix: Vec<String>,
    },
    Inconnue,
}

#[derive(Clone, Debug)]
pub struct Missing {
    pub path: Vec<usize>,
    pub field: &'static str,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct Proposal {
    pub steps: Vec<Step>,
    pub notes: Vec<String>,
    pub missing: Vec<Missing>,
    pub status: Status,
}

impl Proposal {
    pub fn insertable(&self) -> bool {
        self.status == Status::Reconnue && self.missing.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub reason: String,
    pub steps: Vec<Step>,
}
