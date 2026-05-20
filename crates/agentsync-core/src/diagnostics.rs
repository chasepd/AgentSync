use crate::model::{Agent, ResourceKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub resource_id: Option<String>,
    pub resource_kind: Option<ResourceKind>,
    pub agent: Option<Agent>,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentSyncError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("adapter error: {0}")]
    Adapter(String),
}

