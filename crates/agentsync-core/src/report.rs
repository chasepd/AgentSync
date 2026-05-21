use crate::diagnostics::Diagnostic;
use crate::model::{NativeResource, NormalizedResource, RenderedFile, Scope};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScanReport {
    pub scope: Scope,
    pub resources: Vec<NativeResource>,
    pub normalized: Vec<NormalizedResource>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ScanReport {
    pub fn empty(scope: Scope) -> Self {
        Self {
            scope,
            resources: Vec::new(),
            normalized: Vec::new(),
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
        !self
            .items
            .iter()
            .all(|item| item.state == DriftState::Clean)
            || !self.diagnostics.is_empty()
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
        }
        for diagnostic in &self.diagnostics {
            out.push_str(&format!(
                "{:?}: {}\n",
                diagnostic.severity, diagnostic.message
            ));
        }
        if self.has_blocking_issues() {
            out.push_str("Suggested action: run agentsync diff rules --from agents-md --to claude,cursor,opencode before writing.\n");
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
