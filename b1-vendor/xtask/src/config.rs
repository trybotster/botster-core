//! `contracts.toml`: the current contract set and the clause overrides.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    /// The directory of the contract files, relative to the repo root.
    pub dir: String,
    /// The contracts in owner order.
    #[serde(rename = "contract")]
    pub contracts: Vec<Contract>,
    pub superseded: Superseded,
    #[serde(rename = "clause_override", default)]
    pub overrides: Vec<ClauseOverride>,
}

#[derive(Debug, Deserialize)]
pub struct Contract {
    pub prefix: String,
    pub files: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Superseded {
    pub files: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClauseOverride {
    pub id_prefix: String,
    pub clause: String,
    #[allow(dead_code)]
    pub note: String,
}

impl Config {
    pub fn load(root: &Path) -> Result<Config> {
        let path = root.join("contracts.toml");
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
    }
}
