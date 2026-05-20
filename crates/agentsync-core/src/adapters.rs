use std::path::Path;

use crate::diagnostics::{AgentSyncError, Diagnostic};
use crate::model::{
    AdapterCapabilities, NativeResource, NormalizedResource, RenderedFile, Scope,
};

pub trait AgentAdapter {
    fn capabilities(&self) -> AdapterCapabilities;

    fn discover(&self, root: &Path, scope: Scope) -> Result<Vec<NativeResource>, AgentSyncError>;

    fn read(&self, native: &NativeResource) -> Result<NormalizedResource, AgentSyncError>;

    fn render(
        &self,
        resource: &NormalizedResource,
    ) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError>;

    fn validate(&self, root: &Path, scope: Scope) -> Result<Vec<Diagnostic>, AgentSyncError>;
}

