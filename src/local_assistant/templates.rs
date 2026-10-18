use crate::catalog;
use crate::model::Step;

/// Modèles existants, clonés sans créer d'action ou de valeur présumée.
pub fn common() -> Vec<(String, Vec<Step>)> {
    catalog::presets()
        .into_iter()
        .filter(|p| ["Média", "OBS", "Système", "Clavier"].contains(&p.category))
        .map(|p| (p.key.label, p.key.steps))
        .collect()
}
