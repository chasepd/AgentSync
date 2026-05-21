use crate::model::{Agent, ResourceKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

    #[error("serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}
