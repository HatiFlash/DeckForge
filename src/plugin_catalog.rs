//! Offline, versioned plugin metadata. No third-party bundle is published here.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OriginKind {
    Native,
    Compatible,
    Converted,
    Local,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Origin {
    pub kind: OriginKind,
    pub repository: Option<String>,
    pub revision: Option<String>,
    pub source_sha256: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conversion {
    pub adapter: String,
    pub adapter_version: String,
    pub author: Option<String>,
    pub source_version: Option<String>,
    pub report: Option<String>,
    pub known_differences: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    Unverified,
    Compatible,
    Adaptable,
    Partial,
    Unsupported,
    Protected,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Installation {
    Absent,
    InstalledInactive,
    Active,
    UpdateAvailable,
    MissingRuntime,
    Incompatible,
    Error,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Redistribution {
    Unreviewed,
    Allowed,
    Forbidden,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeRequirement {
    pub kind: String,
    pub requirement: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub upstream_id: Option<String>,
    pub name: String,
    pub author: String,
    pub description: String,
    pub icon: Option<String>,
    pub format: String,
    pub version: String,
    pub origin: Origin,
    pub conversion: Option<Conversion>,
    pub platforms: Vec<String>,
    pub runtime: RuntimeRequirement,
    pub capabilities: Vec<String>,
    pub redistribution: Redistribution,
    pub compatibility: Compatibility,
    pub tested_at: Option<String>,
    pub installation: Installation,
    pub actions: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Catalog {
    pub schema_version: u32,
    pub entries: Vec<Entry>,
}
impl Catalog {
    pub fn offline() -> Self {
        Self::parse(br#"{"schema_version":1,"entries":[]}"#).expect("catalogue embarqué valide")
    }
    pub fn parse(bytes: &[u8]) -> anyhow::Result<Self> {
        let catalog: Self = serde_json::from_slice(bytes)?;
        anyhow::ensure!(catalog.schema_version == 1, "Version de catalogue inconnue");
        Ok(catalog)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offline_catalog_roundtrip() {
        let bytes = serde_json::to_vec(&Catalog::offline()).unwrap();
        assert!(Catalog::parse(&bytes).unwrap().entries.is_empty());
        assert!(Catalog::parse(br#"{"schema_version":2,"entries":[]}"#).is_err());
    }
}
