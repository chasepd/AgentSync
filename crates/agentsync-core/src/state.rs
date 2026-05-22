use std::fs;
use std::path::{Path, PathBuf};

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diagnostics::AgentSyncError;
use crate::diagnostics::Diagnostic;
use crate::model::{Agent, ResourceKind};

const STATE_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StateFile {
    pub version: u32,
    pub resources: Vec<StateResource>,
}

impl StateFile {
    pub fn empty() -> Self {
        Self {
            version: STATE_VERSION,
            resources: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), AgentSyncError> {
        if self.version != STATE_VERSION {
            return Err(AgentSyncError::InvalidArgument(format!(
                "unsupported state version {}; expected {STATE_VERSION}",
                self.version
            )));
        }
        Ok(())
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
    #[serde(default)]
    pub source_agent: Option<Agent>,
    pub source_paths: Vec<PathBuf>,
    pub source_checksum: String,
    pub targets: Vec<StateTarget>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    pub native_extensions: BTreeMap<String, serde_json::Value>,
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
    let state = serde_json::from_str::<StateFile>(&fs::read_to_string(path)?)?;
    state.validate()?;
    Ok(state)
}

pub fn save_state(root: &Path, state: &StateFile) -> Result<(), AgentSyncError> {
    let path = root.join(".agentsync/state.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(state)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_state_accepts_legacy_resources_without_metadata() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
        fs::write(
            dir.path().join(".agentsync/state.json"),
            r#"{
  "version": 1,
  "resources": [
    {
      "resource_id": "rules:agents-md",
      "kind": "rule_set",
      "source_paths": ["AGENTS.md"],
      "source_checksum": "abc",
      "targets": [],
      "last_synced_at": "2026-05-22T00:00:00Z"
    }
  ]
}"#,
        )
        .unwrap();

        let state = load_state(dir.path()).unwrap();
        let entry = &state.resources[0];

        assert_eq!(entry.source_agent, None);
        assert!(entry.diagnostics.is_empty());
        assert!(entry.native_extensions.is_empty());
    }

    #[test]
    fn unsupported_state_version_fails() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".agentsync")).unwrap();
        fs::write(
            dir.path().join(".agentsync/state.json"),
            r#"{"version":2,"resources":[]}"#,
        )
        .unwrap();

        assert!(load_state(dir.path())
            .unwrap_err()
            .to_string()
            .contains("unsupported state version 2; expected 1"));
    }
}
