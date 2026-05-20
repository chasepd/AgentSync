use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scope {
    Project,
    User,
    All,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Agent {
    Claude,
    Codex,
    Cursor,
    OpenCode,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceKind {
    RuleSet,
    Skill,
    Subagent,
    Hook,
    Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportLevel {
    Read,
    Write,
    Sync,
    Partial,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeResource {
    pub agent: Agent,
    pub kind: ResourceKind,
    pub scope: Scope,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedResource {
    pub id: String,
    pub kind: ResourceKind,
    pub scope: Scope,
    pub source_agent: Agent,
    pub native_paths: Vec<PathBuf>,
    pub portable_fields: BTreeMap<String, String>,
    pub native_extensions: BTreeMap<Agent, BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedFile {
    pub path: PathBuf,
    pub contents: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterCapabilities {
    pub agent: Agent,
    pub resources: BTreeMap<ResourceKind, SupportLevel>,
    pub fields: BTreeMap<String, SupportLevel>,
}

