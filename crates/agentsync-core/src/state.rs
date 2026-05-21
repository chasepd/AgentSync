use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::diagnostics::AgentSyncError;
use crate::model::{Agent, ResourceKind};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StateFile {
    pub version: u32,
    pub resources: Vec<StateResource>,
}

impl StateFile {
    pub fn empty() -> Self {
        Self {
            version: 1,
            resources: Vec::new(),
        }
    }

    pub fn resource_mut(&mut self, resource_id: &str) -> Option<&mut StateResource> {
        self.resources
            .iter_mut()
            .find(|entry| entry.resource_id == resource_id)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StateResource {
    pub resource_id: String,
    pub kind: ResourceKind,
    pub source_paths: Vec<PathBuf>,
    pub source_checksum: String,
    pub targets: Vec<StateTarget>,
    pub last_synced_at: String,
}

impl StateResource {
    pub fn upsert_target(&mut self, target: StateTarget) {
        self.targets.retain(|existing| existing.path != target.path);
        self.targets.push(target);
        self.targets.sort_by(|a, b| a.path.cmp(&b.path));
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StateTarget {
    pub agent: Agent,
    pub path: PathBuf,
    pub native_checksum: String,
}

pub fn load_state(root: &Path) -> Result<StateFile, AgentSyncError> {
    let path = root.join(".agentsync/state.json");
    if !path.exists() {
        return Ok(StateFile::empty());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn save_state(root: &Path, state: &StateFile) -> Result<(), AgentSyncError> {
    let path = root.join(".agentsync/state.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(state)?)?;
    Ok(())
}
