use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Project,
    User,
    All,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Agent {
    Claude,
    Codex,
    #[serde(rename = "cursor")]
    CursorCli,
    #[serde(rename = "opencode")]
    OpenCode,
}

impl Agent {
    pub const ALL: [Self; 4] = [Self::Codex, Self::Claude, Self::CursorCli, Self::OpenCode];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::CursorCli => "cursor",
            Self::OpenCode => "opencode",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex CLI",
            Self::CursorCli => "Cursor CLI",
            Self::OpenCode => "OpenCode",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    RuleSet,
    Skill,
    Subagent,
    Hook,
    Command,
    Plugin,
    Permission,
}

impl ResourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RuleSet => "rules",
            Self::Skill => "skills",
            Self::Subagent => "subagents",
            Self::Hook => "hooks",
            Self::Command => "commands",
            Self::Plugin => "plugins",
            Self::Permission => "permissions",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportLevel {
    Portable,
    Partial,
    Blocked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NativeResource {
    pub id: String,
    pub agent: Agent,
    pub kind: ResourceKind,
    pub scope: Scope,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiscoveryRoots {
    pub project: PathBuf,
    pub user: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NormalizedResource {
    pub id: String,
    pub kind: ResourceKind,
    pub scope: Scope,
    pub source_agent: Agent,
    pub native_paths: Vec<PathBuf>,
    pub rule_set: Option<RuleSet>,
    pub skill: Option<Skill>,
    pub subagent: Option<Subagent>,
    #[serde(default)]
    pub command: Option<CommandDefinition>,
    pub native_extensions: BTreeMap<String, serde_json::Value>,
    pub diagnostics: Vec<crate::diagnostics::Diagnostic>,
    pub support: SupportLevel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuleSet {
    pub body: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Skill {
    pub name: String,
    pub description: Option<String>,
    pub body: String,
    pub asset_paths: Vec<PathBuf>,
    pub assets: Vec<SkillAsset>,
    pub frontmatter: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SkillAsset {
    pub relative_path: PathBuf,
    pub contents: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Subagent {
    pub name: String,
    pub description: Option<String>,
    pub body: String,
    #[serde(default)]
    pub instructions: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub tools: ToolPolicy,
    #[serde(default)]
    pub permissions: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub mode: Option<String>,
    pub frontmatter: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolPolicy {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub native: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandDefinition {
    pub name: String,
    pub template: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub subtask: Option<bool>,
    pub frontmatter: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RenderedFile {
    pub path: PathBuf,
    pub contents: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdapterCapabilities {
    pub agent: Agent,
    pub resources: BTreeMap<ResourceKind, SupportLevel>,
    pub fields: BTreeMap<String, SupportLevel>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceSelector {
    Rules,
    Skills,
    Subagents,
    Commands,
    Hooks,
}

impl ResourceSelector {
    pub const ALL: [Self; 5] = [
        Self::Rules,
        Self::Skills,
        Self::Subagents,
        Self::Commands,
        Self::Hooks,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rules => "rules",
            Self::Skills => "skills",
            Self::Subagents => "subagents",
            Self::Commands => "commands",
            Self::Hooks => "hooks",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceFilter {
    pub selector: ResourceSelector,
    pub name: Option<String>,
}

impl ResourceFilter {
    pub fn all(selector: ResourceSelector) -> Self {
        Self {
            selector,
            name: None,
        }
    }

    pub fn named(selector: ResourceSelector, name: impl Into<String>) -> Self {
        Self {
            selector,
            name: Some(name.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlanOptions {
    #[serde(default)]
    pub no_overwrite: bool,
    #[serde(default)]
    pub strategy: ConflictStrategy,
}

impl Default for PlanOptions {
    fn default() -> Self {
        Self {
            no_overwrite: false,
            strategy: ConflictStrategy::Conservative,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStrategy {
    #[default]
    Conservative,
    Source,
    Newest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceAlias {
    All,
    AgentsMd,
    Codex,
    Claude,
    #[serde(rename = "cursor")]
    CursorCli,
    OpenCode,
}
