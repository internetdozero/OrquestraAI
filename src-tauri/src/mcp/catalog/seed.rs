//! Shape of one entry in the audited first-run dataset.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub(super) struct SeedEntry {
    #[serde(default)]
    pub(super) id: String,
    pub(super) name: String,
    #[serde(default)]
    pub(super) description: String,
    pub(super) command: String,
    #[serde(default)]
    pub(super) args: Vec<String>,
    #[serde(default)]
    pub(super) env: HashMap<String, String>,
    #[serde(default)]
    pub(super) url: Option<String>,
    #[serde(default)]
    pub(super) headers: HashMap<String, String>,
    #[serde(default)]
    pub(super) enabled: bool,
}
