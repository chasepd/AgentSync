pub mod adapters;
pub mod diagnostics;
pub mod model;
pub mod report;

pub use diagnostics::{AgentSyncError, Diagnostic, DiagnosticSeverity};
pub use model::{Agent, ResourceKind, Scope, SupportLevel};
pub use report::ScanReport;

pub fn scan(scope: Scope) -> Result<ScanReport, AgentSyncError> {
    Ok(ScanReport::empty(scope))
}

pub fn status(scope: Scope) -> Result<ScanReport, AgentSyncError> {
    Ok(ScanReport::empty(scope))
}

