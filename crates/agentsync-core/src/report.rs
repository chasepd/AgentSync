use crate::diagnostics::Diagnostic;
use crate::model::{NativeResource, Scope};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanReport {
    pub scope: Scope,
    pub resources: Vec<NativeResource>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ScanReport {
    pub fn empty(scope: Scope) -> Self {
        Self {
            scope,
            resources: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn has_drift(&self) -> bool {
        false
    }

    pub fn to_table(&self) -> String {
        format!(
            "AgentSync scan ({:?}): {} resources, {} diagnostics",
            self.scope,
            self.resources.len(),
            self.diagnostics.len()
        )
    }

    pub fn to_json_placeholder(&self) -> String {
        format!(
            "{{\"scope\":\"{:?}\",\"resources\":[],\"diagnostics\":[]}}",
            self.scope
        )
    }
}

