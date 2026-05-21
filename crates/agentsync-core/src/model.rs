use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
}

impl ResourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RuleSet => "rules",
            Self::Skill => "skills",
            Self::Subagent => "subagents",
            Self::Hook => "hooks",
            Self::Command => "commands",
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
pub struct NormalizedResource {
    pub id: String,
    pub kind: ResourceKind,
    pub scope: Scope,
    pub source_agent: Agent,
    pub native_paths: Vec<PathBuf>,
    pub rule_set: Option<RuleSet>,
    pub skill: Option<Skill>,
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
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceAlias {
    AgentsMd,
    Codex,
    Claude,
    #[serde(rename = "cursor")]
    CursorCli,
    OpenCode,
}
