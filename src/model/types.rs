use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub(crate) struct SkillEntry {
    pub destination: String,
    pub hash: String,
    pub skill: String,
    #[serde(default)]
    pub description: String,
    pub source: String,
    pub source_revision: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct State {
    pub version: u8,
    pub last_run: Option<String>,
    pub skills: BTreeMap<String, SkillEntry>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            last_run: None,
            skills: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Serialize, Default)]
pub(crate) struct Summary {
    pub installed: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub broken: usize,
    pub failed: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct Action {
    pub source: Option<String>,
    pub skill: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Report {
    pub run_id: String,
    pub config: String,
    pub destination: String,
    pub dry_run: bool,
    pub summary: Summary,
    pub actions: Vec<Action>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct InstalledSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub skill: String,
    pub destination: String,
    pub hash: String,
    pub source_revision: String,
    pub updated_at: String,
    pub updated_ago: String,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct SyncFailure {
    pub name: String,
    pub source: String,
    pub reason: String,
}
