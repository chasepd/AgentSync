use crate::diagnostics::Diagnostic;
use crate::model::{AdapterCapabilities, NativeResource, NormalizedResource, RenderedFile, Scope};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScanReport {
    pub scope: Scope,
    pub resources: Vec<NativeResource>,
    pub normalized: Vec<NormalizedResource>,
    pub capabilities: Vec<AdapterCapabilities>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ScanReport {
    pub fn empty(scope: Scope) -> Self {
        Self {
            scope,
            resources: Vec::new(),
            normalized: Vec::new(),
            capabilities: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn to_table(&self) -> String {
        let mut out = format!("AgentSync scan ({:?})\n", self.scope);
        for resource in &self.resources {
            out.push_str(&format!(
                "{:<10} {:<8} {}\n",
                resource.kind.as_str(),
                resource.agent.as_str(),
                resource.path.display()
            ));
        }
        for diagnostic in &self.diagnostics {
            out.push_str(&format!(
                "{:?}: {}\n",
                diagnostic.severity, diagnostic.message
            ));
        }
        out
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StatusReport {
    pub scope: Scope,
    pub items: Vec<StatusItem>,
    pub diagnostics: Vec<Diagnostic>,
}

impl StatusReport {
    pub fn has_blocking_issues(&self) -> bool {
        self.items.iter().any(StatusItem::is_blocking)
            || self.diagnostics.iter().any(Diagnostic::is_blocking)
    }

    pub fn to_table(&self) -> String {
        let mut out = format!("AgentSync status ({:?})\n", self.scope);
        if self.items.is_empty() && self.diagnostics.is_empty() {
            out.push_str("No tracked resources. Run sync --write to create state.\n");
            return out;
        }
        for item in &self.items {
            out.push_str(&format!(
                "{:<12} {:<10} {:?} {}\n",
                item.kind.as_str(),
                item.id,
                item.state,
                item.message
            ));
            if let Some(command) = &item.suggested_command {
                out.push_str(&format!("  suggested: {command}\n"));
            }
        }
        for diagnostic in &self.diagnostics {
            out.push_str(&format!(
                "{:?}: {}\n",
                diagnostic.severity, diagnostic.message
            ));
        }
        if self.has_blocking_issues() {
            out.push_str("Suggested action: inspect the specific command above before writing.\n");
        }
        out
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StatusItem {
    pub id: String,
    pub kind: crate::model::ResourceKind,
    pub state: DriftState,
    pub message: String,
    pub suggested_command: Option<String>,
}

impl StatusItem {
    pub fn is_blocking(&self) -> bool {
        matches!(
            self.state,
            DriftState::MissingTarget
                | DriftState::StaleTarget
                | DriftState::ChangedSource
                | DriftState::Blocked
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftState {
    Clean,
    Untracked,
    MissingTarget,
    StaleTarget,
    ChangedSource,
    Blocked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlanReport {
    pub actions: Vec<PlanAction>,
    pub diagnostics: Vec<Diagnostic>,
}

impl PlanReport {
    pub fn has_blocked_actions(&self) -> bool {
        self.actions
            .iter()
            .any(|action| action.action == PlanActionKind::Block)
    }

    pub fn has_blocking_diagnostics(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_blocking)
    }

    pub fn has_blocking_issues(&self) -> bool {
        self.has_blocked_actions() || self.has_blocking_diagnostics()
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for action in &self.actions {
            out.push_str(&format!(
                "{:?} {} ({})\n",
                action.action,
                action.path.display(),
                action.reason
            ));
            if let Some(diff) = &action.diff {
                out.push_str(diff);
                if !diff.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
        for diagnostic in &self.diagnostics {
            out.push_str(&format!(
                "{:?}: {}\n",
                diagnostic.severity, diagnostic.message
            ));
        }
        out
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlanAction {
    pub action: PlanActionKind,
    pub path: std::path::PathBuf,
    pub resource_id: String,
    pub reason: String,
    pub rendered: Option<RenderedFile>,
    pub diff: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanActionKind {
    Create,
    Update,
    Skip,
    Block,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DoctorReport {
    pub root: PathBuf,
    pub state_path: PathBuf,
    pub state_present: bool,
    pub state_valid: bool,
    pub resource_count: usize,
    pub normalized_count: usize,
    pub tracked_resource_count: usize,
    pub blocking_status_count: usize,
    pub diagnostics: Vec<Diagnostic>,
}

impl DoctorReport {
    pub fn has_blocking_issues(&self) -> bool {
        !self.state_valid
            || self.blocking_status_count > 0
            || self.diagnostics.iter().any(Diagnostic::is_blocking)
    }

    pub fn to_text(&self) -> String {
        let mut out = format!("AgentSync doctor ({})\n", self.root.display());
        out.push_str(&format!("state: {}\n", self.state_path.display()));
        out.push_str(&format!(
            "state_status: {}\n",
            if self.state_present {
                if self.state_valid {
                    "valid"
                } else {
                    "invalid"
                }
            } else {
                "missing"
            }
        ));
        out.push_str(&format!("resources: {}\n", self.resource_count));
        out.push_str(&format!("normalized: {}\n", self.normalized_count));
        out.push_str(&format!("tracked: {}\n", self.tracked_resource_count));
        out.push_str(&format!(
            "blocking_status: {}\n",
            self.blocking_status_count
        ));
        for diagnostic in &self.diagnostics {
            out.push_str(&format!(
                "{:?}: {}\n",
                diagnostic.severity, diagnostic.message
            ));
        }
        out
    }
}
