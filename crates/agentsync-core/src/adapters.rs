use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use jsonc_parser::{errors::ParseError, parse_to_serde_value, ParseOptions};
use serde_json::Value;
use walkdir::WalkDir;

use crate::diagnostics::{AgentSyncError, Diagnostic, DiagnosticSeverity};
use crate::model::{
    AdapterCapabilities, Agent, CommandDefinition, NativeResource, NormalizedResource,
    RenderedFile, ResourceKind, RuleSet, Scope, Skill, SkillAsset, Subagent, SupportLevel,
    ToolPolicy,
};

pub trait AgentAdapter {
    fn agent(&self) -> Agent;

    fn capabilities(&self) -> AdapterCapabilities;

    fn discover(&self, root: &Path, scope: Scope) -> Result<Vec<NativeResource>, AgentSyncError>;

    fn read(
        &self,
        root: &Path,
        native: &NativeResource,
    ) -> Result<NormalizedResource, AgentSyncError>;

    fn render(
        &self,
        resource: &NormalizedResource,
    ) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError>;

    fn validate(&self, _root: &Path, _scope: Scope) -> Result<Vec<Diagnostic>, AgentSyncError> {
        Ok(Vec::new())
    }
}

pub struct AdapterRegistry {
    adapters: Vec<Box<dyn AgentAdapter>>,
}

impl AdapterRegistry {
    pub fn empty() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    pub fn built_in() -> Self {
        Self {
            adapters: built_in_adapters(),
        }
    }

    pub fn from_adapters(adapters: Vec<Box<dyn AgentAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn register(&mut self, adapter: Box<dyn AgentAdapter>) {
        self.adapters.push(adapter);
    }

    pub fn adapters(&self) -> &[Box<dyn AgentAdapter>] {
        &self.adapters
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::built_in()
    }
}

pub fn built_in_adapters() -> Vec<Box<dyn AgentAdapter>> {
    vec![
        Box::new(CodexAdapter),
        Box::new(ClaudeAdapter),
        Box::new(CursorCliAdapter),
        Box::new(OpenCodeAdapter),
    ]
}

#[derive(Debug)]
pub struct CodexAdapter;

impl AgentAdapter for CodexAdapter {
    fn agent(&self) -> Agent {
        Agent::Codex
    }

    fn capabilities(&self) -> AdapterCapabilities {
        portable_capabilities(Agent::Codex)
    }

    fn discover(&self, root: &Path, scope: Scope) -> Result<Vec<NativeResource>, AgentSyncError> {
        let mut resources = Vec::new();
        let (rules, skills, hook_files) = match scope {
            Scope::Project => (
                "AGENTS.md",
                [".codex/skills", ".agents/skills"],
                [".codex/hooks.json"],
            ),
            Scope::User => (
                ".codex/AGENTS.md",
                [".codex/skills", ".agents/skills"],
                [".codex/hooks.json"],
            ),
            Scope::All => return Ok(Vec::new()),
        };
        push_if_exists(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::RuleSet,
            rules,
            scope,
        );
        discover_skill_dirs(&mut resources, root, self.agent(), skills, scope);
        discover_json_key_files(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Hook,
            hook_files,
            "hooks",
            scope,
        );
        Ok(resources)
    }

    fn read(
        &self,
        root: &Path,
        native: &NativeResource,
    ) -> Result<NormalizedResource, AgentSyncError> {
        normalize_native(root, native)
    }

    fn render(
        &self,
        resource: &NormalizedResource,
    ) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
        render_native(resource, Agent::Codex)
    }
}

#[derive(Debug)]
pub struct ClaudeAdapter;

impl AgentAdapter for ClaudeAdapter {
    fn agent(&self) -> Agent {
        Agent::Claude
    }

    fn capabilities(&self) -> AdapterCapabilities {
        portable_capabilities(Agent::Claude)
    }

    fn discover(&self, root: &Path, scope: Scope) -> Result<Vec<NativeResource>, AgentSyncError> {
        let mut resources = Vec::new();
        if scope == Scope::All {
            return Ok(resources);
        }
        push_if_exists(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::RuleSet,
            "CLAUDE.md",
            scope,
        );
        push_if_exists(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::RuleSet,
            ".claude/CLAUDE.md",
            scope,
        );
        discover_skill_dirs(
            &mut resources,
            root,
            self.agent(),
            [".claude/skills"],
            scope,
        );
        discover_md_dir(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Subagent,
            ".claude/agents",
            scope,
        );
        discover_json_key_files(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Hook,
            [".claude/settings.json", ".claude/settings.local.json"],
            "hooks",
            scope,
        );
        discover_json_key_files(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Permission,
            [".claude/settings.json", ".claude/settings.local.json"],
            "permissions",
            scope,
        );
        Ok(resources)
    }

    fn read(
        &self,
        root: &Path,
        native: &NativeResource,
    ) -> Result<NormalizedResource, AgentSyncError> {
        normalize_native(root, native)
    }

    fn render(
        &self,
        resource: &NormalizedResource,
    ) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
        render_native(resource, Agent::Claude)
    }
}

#[derive(Debug)]
pub struct CursorCliAdapter;

impl AgentAdapter for CursorCliAdapter {
    fn agent(&self) -> Agent {
        Agent::CursorCli
    }

    fn capabilities(&self) -> AdapterCapabilities {
        portable_capabilities(Agent::CursorCli)
    }

    fn discover(&self, root: &Path, scope: Scope) -> Result<Vec<NativeResource>, AgentSyncError> {
        if scope != Scope::Project {
            return Ok(Vec::new());
        }
        let mut resources = Vec::new();
        push_if_exists(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::RuleSet,
            "AGENTS.md",
            scope,
        );
        discover_md_dir(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::RuleSet,
            ".cursor/rules",
            scope,
        );
        discover_skill_dirs(
            &mut resources,
            root,
            self.agent(),
            [".cursor/skills"],
            scope,
        );
        discover_json_key_files(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Hook,
            [".cursor/hooks.json"],
            "hooks",
            scope,
        );
        Ok(resources)
    }

    fn read(
        &self,
        root: &Path,
        native: &NativeResource,
    ) -> Result<NormalizedResource, AgentSyncError> {
        normalize_native(root, native)
    }

    fn render(
        &self,
        resource: &NormalizedResource,
    ) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
        render_native(resource, Agent::CursorCli)
    }
}

#[derive(Debug)]
pub struct OpenCodeAdapter;

impl AgentAdapter for OpenCodeAdapter {
    fn agent(&self) -> Agent {
        Agent::OpenCode
    }

    fn capabilities(&self) -> AdapterCapabilities {
        portable_capabilities(Agent::OpenCode)
    }

    fn discover(&self, root: &Path, scope: Scope) -> Result<Vec<NativeResource>, AgentSyncError> {
        let mut resources = Vec::new();
        let (rules, config_files, skill_dirs, agent_dir, command_dir, plugin_dir) = match scope {
            Scope::Project => (
                "AGENTS.md",
                OPENCODE_CONFIG_FILES,
                [".opencode/skills"],
                ".opencode/agents",
                ".opencode/commands",
                ".opencode/plugins",
            ),
            Scope::User => (
                ".config/opencode/AGENTS.md",
                OPENCODE_USER_CONFIG_FILES,
                [".config/opencode/skills"],
                ".config/opencode/agents",
                ".config/opencode/commands",
                ".config/opencode/plugins",
            ),
            Scope::All => return Ok(resources),
        };
        push_if_exists(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::RuleSet,
            rules,
            scope,
        );
        for file in config_files {
            push_if_exists(
                &mut resources,
                root,
                self.agent(),
                ResourceKind::RuleSet,
                file,
                scope,
            );
        }
        discover_skill_dirs(&mut resources, root, self.agent(), skill_dirs, scope);
        discover_md_dir(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Subagent,
            agent_dir,
            scope,
        );
        discover_json_key_files(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Subagent,
            config_files,
            "agent",
            scope,
        );
        discover_md_dir(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Command,
            command_dir,
            scope,
        );
        discover_opencode_config_commands(&mut resources, root, config_files, scope);
        discover_ext_dir(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Plugin,
            plugin_dir,
            &["cjs", "cts", "js", "mjs", "mts", "ts"],
            scope,
        );
        discover_json_key_files(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Plugin,
            config_files,
            "plugin",
            scope,
        );
        discover_json_key_files(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Permission,
            config_files,
            "permission",
            scope,
        );
        Ok(resources)
    }

    fn read(
        &self,
        root: &Path,
        native: &NativeResource,
    ) -> Result<NormalizedResource, AgentSyncError> {
        normalize_native(root, native)
    }

    fn render(
        &self,
        resource: &NormalizedResource,
    ) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
        render_native(resource, Agent::OpenCode)
    }
}

const OPENCODE_CONFIG_FILES: [&str; 2] = ["opencode.json", "opencode.jsonc"];
const OPENCODE_USER_CONFIG_FILES: [&str; 2] = [
    ".config/opencode/opencode.json",
    ".config/opencode/opencode.jsonc",
];

fn portable_capabilities(agent: Agent) -> AdapterCapabilities {
    let mut resources = BTreeMap::new();
    resources.insert(ResourceKind::RuleSet, SupportLevel::Portable);
    resources.insert(ResourceKind::Skill, SupportLevel::Portable);
    resources.insert(ResourceKind::Subagent, SupportLevel::Partial);
    resources.insert(
        ResourceKind::Hook,
        if matches!(
            agent,
            Agent::Codex | Agent::Claude | Agent::CursorCli | Agent::OpenCode
        ) {
            SupportLevel::Partial
        } else {
            SupportLevel::Blocked
        },
    );
    resources.insert(ResourceKind::Command, SupportLevel::Partial);
    resources.insert(ResourceKind::Plugin, SupportLevel::Blocked);
    resources.insert(ResourceKind::Permission, SupportLevel::Blocked);

    let mut fields = BTreeMap::new();
    fields.insert("rules.body".to_string(), SupportLevel::Portable);
    fields.insert("skill.name".to_string(), SupportLevel::Portable);
    fields.insert("skill.description".to_string(), SupportLevel::Portable);
    fields.insert("skill.body".to_string(), SupportLevel::Portable);
    fields.insert("skill.assets".to_string(), SupportLevel::Partial);
    fields.insert("subagent.name".to_string(), SupportLevel::Portable);
    fields.insert("subagent.description".to_string(), SupportLevel::Portable);
    fields.insert("subagent.instructions".to_string(), SupportLevel::Portable);
    fields.insert("subagent.model".to_string(), SupportLevel::Partial);
    fields.insert("subagent.effort".to_string(), SupportLevel::Partial);
    fields.insert("subagent.mode".to_string(), SupportLevel::Partial);
    fields.insert("subagent.tools".to_string(), SupportLevel::Blocked);
    fields.insert("subagent.permissions".to_string(), SupportLevel::Blocked);
    fields.insert("command.name".to_string(), SupportLevel::Portable);
    fields.insert("command.template".to_string(), SupportLevel::Portable);
    fields.insert("command.description".to_string(), SupportLevel::Portable);
    fields.insert("command.agent".to_string(), SupportLevel::Blocked);
    fields.insert("command.model".to_string(), SupportLevel::Partial);
    fields.insert("command.subtask".to_string(), SupportLevel::Blocked);
    fields.insert("command.shell_output".to_string(), SupportLevel::Blocked);
    fields.insert("behavior.executable".to_string(), SupportLevel::Blocked);

    AdapterCapabilities {
        agent,
        resources,
        fields,
    }
}

fn push_if_exists(
    resources: &mut Vec<NativeResource>,
    root: &Path,
    agent: Agent,
    kind: ResourceKind,
    rel: &str,
    scope: Scope,
) {
    if root.join(rel).is_file() {
        resources.push(native(agent, kind, rel, scope));
    }
}

fn discover_skill_dirs<const N: usize>(
    resources: &mut Vec<NativeResource>,
    root: &Path,
    agent: Agent,
    dirs: [&str; N],
    scope: Scope,
) {
    for dir in dirs {
        if let Ok(entries) = fs::read_dir(root.join(dir)) {
            for entry in entries.flatten() {
                let skill = entry.path().join("SKILL.md");
                if skill.is_file() {
                    if let Ok(rel) = skill.strip_prefix(root) {
                        resources.push(native(
                            agent,
                            ResourceKind::Skill,
                            &rel.to_string_lossy(),
                            scope,
                        ));
                    }
                }
            }
        }
    }
}

fn discover_md_dir(
    resources: &mut Vec<NativeResource>,
    root: &Path,
    agent: Agent,
    kind: ResourceKind,
    dir: &str,
    scope: Scope,
) {
    let abs = root.join(dir);
    if !abs.exists() {
        return;
    }
    for entry in WalkDir::new(abs).into_iter().flatten() {
        if entry.file_type().is_file() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                if let Ok(rel) = path.strip_prefix(root) {
                    resources.push(native(agent, kind, &rel.to_string_lossy(), scope));
                }
            }
        }
    }
}

fn discover_ext_dir(
    resources: &mut Vec<NativeResource>,
    root: &Path,
    agent: Agent,
    kind: ResourceKind,
    dir: &str,
    extensions: &[&str],
    scope: Scope,
) {
    let abs = root.join(dir);
    if !abs.exists() {
        return;
    }
    for entry in WalkDir::new(abs).into_iter().flatten() {
        if entry.file_type().is_file() {
            let path = entry.path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| extensions.contains(&ext))
            {
                if let Ok(rel) = path.strip_prefix(root) {
                    if agent == Agent::OpenCode
                        && kind == ResourceKind::Plugin
                        && is_agentsync_generated_opencode_hook_plugin(path)
                    {
                        continue;
                    }
                    resources.push(native(agent, kind, &rel.to_string_lossy(), scope));
                }
            }
        }
    }
}

fn is_agentsync_generated_opencode_hook_plugin(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "agentsync-hooks.js")
        && fs::read_to_string(path).is_ok_and(|raw| {
            raw.starts_with("// Generated by AgentSync from Codex/Claude/Cursor hook config.")
        })
}

fn discover_json_key_files<const N: usize>(
    resources: &mut Vec<NativeResource>,
    root: &Path,
    agent: Agent,
    kind: ResourceKind,
    files: [&str; N],
    key: &str,
    scope: Scope,
) {
    for file in files {
        let abs = root.join(file);
        if !abs.is_file() {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&abs) else {
            continue;
        };
        let Ok(value) = parse_jsonc_value(&raw) else {
            continue;
        };
        if value.get(key).is_some() {
            resources.push(native(agent, kind, file, scope));
        }
    }
}

fn discover_opencode_config_commands<const N: usize>(
    resources: &mut Vec<NativeResource>,
    root: &Path,
    files: [&str; N],
    scope: Scope,
) {
    for file in files {
        let abs = root.join(file);
        if !abs.is_file() {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&abs) else {
            continue;
        };
        let Ok(value) = parse_jsonc_value(&raw) else {
            continue;
        };
        let Some(commands) = value.get("command").and_then(Value::as_object) else {
            continue;
        };
        for name in commands.keys() {
            resources.push(NativeResource {
                id: format!("commands:opencode:{}:{name}", file.replace('\\', "/")),
                agent: Agent::OpenCode,
                kind: ResourceKind::Command,
                scope,
                path: PathBuf::from(file),
            });
        }
    }
}

fn native(agent: Agent, kind: ResourceKind, rel: &str, scope: Scope) -> NativeResource {
    NativeResource {
        id: format!(
            "{}:{}:{}",
            kind.as_str(),
            agent.as_str(),
            rel.replace('\\', "/")
        ),
        agent,
        kind,
        scope,
        path: PathBuf::from(rel),
    }
}

fn normalize_native(
    root: &Path,
    native: &NativeResource,
) -> Result<NormalizedResource, AgentSyncError> {
    match native.kind {
        ResourceKind::RuleSet => normalize_rule(root, native),
        ResourceKind::Skill => normalize_skill(root, native),
        ResourceKind::Subagent
            if native.agent == Agent::OpenCode && is_opencode_config(&native.path) =>
        {
            Ok(blocked_behavior(root, native))
        }
        ResourceKind::Subagent => normalize_subagent(root, native),
        ResourceKind::Command
            if native.agent == Agent::OpenCode && is_opencode_config(&native.path) =>
        {
            normalize_opencode_config_command(root, native)
        }
        ResourceKind::Command if native.agent == Agent::OpenCode => normalize_command(root, native),
        ResourceKind::Hook
        | ResourceKind::Command
        | ResourceKind::Plugin
        | ResourceKind::Permission => Ok(blocked_behavior(root, native)),
    }
}

fn normalize_rule(
    root: &Path,
    native: &NativeResource,
) -> Result<NormalizedResource, AgentSyncError> {
    if native.agent == Agent::OpenCode && is_opencode_config(&native.path) {
        return normalize_opencode_config_rules(root, native);
    }
    let body = fs::read_to_string(root.join(&native.path))?;
    Ok(NormalizedResource {
        id: if native.path == Path::new("AGENTS.md") {
            "rules:agents-md".to_string()
        } else {
            native.id.clone()
        },
        kind: ResourceKind::RuleSet,
        scope: native.scope,
        source_agent: native.agent,
        native_paths: vec![native.path.clone()],
        rule_set: Some(RuleSet { body }),
        skill: None,
        subagent: None,
        command: None,
        native_extensions: BTreeMap::new(),
        diagnostics: Vec::new(),
        support: SupportLevel::Portable,
    })
}

fn normalize_opencode_config_rules(
    root: &Path,
    native: &NativeResource,
) -> Result<NormalizedResource, AgentSyncError> {
    let config = parse_opencode_config(root, native)?;
    let mut diagnostics = Vec::new();
    let mut support = SupportLevel::Portable;
    let mut body_parts = Vec::new();
    let mut native_paths = vec![native.path.clone()];

    match config.get("instructions") {
        Some(Value::Array(instructions)) => {
            for instruction in instructions {
                let Some(path) = instruction.as_str() else {
                    support = SupportLevel::Partial;
                    diagnostics.push(opencode_instruction_diagnostic(
                        native,
                        "opencode.json contains a non-string instruction entry",
                    ));
                    continue;
                };
                if has_glob_pattern(path) {
                    support = SupportLevel::Partial;
                    diagnostics.push(opencode_instruction_diagnostic(
                        native,
                        &format!(
                            "opencode.json instruction pattern {path:?} uses a glob, which is not expanded in the MVP"
                        ),
                    ));
                    continue;
                }
                let rel = Path::new(path);
                if rel.is_absolute()
                    || rel
                        .components()
                        .any(|component| matches!(component, std::path::Component::ParentDir))
                {
                    support = SupportLevel::Partial;
                    diagnostics.push(opencode_instruction_diagnostic(
                        native,
                        &format!(
                            "opencode.json instruction path {path:?} must stay inside the project"
                        ),
                    ));
                    continue;
                }
                let abs = root.join(rel);
                match fs::read_to_string(&abs) {
                    Ok(contents) => {
                        native_paths.push(rel.to_path_buf());
                        body_parts.push(format!("<!-- {} -->\n{}", rel.display(), contents));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        support = SupportLevel::Partial;
                        diagnostics.push(opencode_instruction_diagnostic(
                            native,
                            &format!("opencode.json instruction file {path:?} was not found"),
                        ));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
                        support = SupportLevel::Partial;
                        diagnostics.push(opencode_instruction_diagnostic(
                            native,
                            &format!("opencode.json instruction file {path:?} is not UTF-8"),
                        ));
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        }
        Some(_) => {
            support = SupportLevel::Partial;
            diagnostics.push(opencode_instruction_diagnostic(
                native,
                "opencode.json instructions field must be an array",
            ));
        }
        None => {
            support = SupportLevel::Partial;
            diagnostics.push(opencode_instruction_diagnostic(
                native,
                "opencode.json does not define instructions",
            ));
        }
    }

    native_paths.sort();
    native_paths.dedup();
    let mut native_extensions = BTreeMap::new();
    native_extensions.insert("opencode.config".to_string(), config);
    Ok(NormalizedResource {
        id: native.id.clone(),
        kind: ResourceKind::RuleSet,
        scope: native.scope,
        source_agent: native.agent,
        native_paths,
        rule_set: Some(RuleSet {
            body: body_parts.join("\n\n"),
        }),
        skill: None,
        subagent: None,
        command: None,
        native_extensions,
        diagnostics,
        support,
    })
}

fn parse_opencode_config(root: &Path, native: &NativeResource) -> Result<Value, AgentSyncError> {
    let raw = fs::read_to_string(root.join(&native.path))?;
    parse_jsonc_value(&raw).map_err(|error| {
        AgentSyncError::Adapter(format!(
            "failed to parse {} as JSONC: {error}",
            native.path.display()
        ))
    })
}

fn parse_jsonc_value(raw: &str) -> Result<Value, ParseError> {
    parse_to_serde_value::<Value>(
        raw,
        &ParseOptions {
            allow_comments: true,
            allow_loose_object_property_names: false,
            allow_trailing_commas: true,
            allow_missing_commas: false,
            allow_single_quoted_strings: false,
            allow_hexadecimal_numbers: false,
            allow_unary_plus_numbers: false,
        },
    )
}

fn is_opencode_config(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "opencode.json" || name == "opencode.jsonc")
}

fn opencode_instruction_diagnostic(native: &NativeResource, message: &str) -> Diagnostic {
    Diagnostic {
        severity: DiagnosticSeverity::Warning,
        resource_id: Some(native.id.clone()),
        resource_kind: Some(ResourceKind::RuleSet),
        agent: Some(native.agent),
        message: message.to_string(),
    }
}

fn has_glob_pattern(path: &str) -> bool {
    path.contains('*') || path.contains('?') || path.contains('[')
}

fn normalize_skill(
    root: &Path,
    native: &NativeResource,
) -> Result<NormalizedResource, AgentSyncError> {
    let path = root.join(&native.path);
    let raw = fs::read_to_string(&path)?;
    let (frontmatter, body) = split_frontmatter(&raw).map_err(|error| {
        AgentSyncError::Adapter(format!(
            "failed to parse frontmatter in {}: {error}",
            native.path.display()
        ))
    })?;
    let name = frontmatter
        .get("name")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            path.parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "skill".to_string());
    let description = frontmatter
        .get("description")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let dir = native.path.parent().unwrap_or(Path::new(""));
    let mut asset_paths = Vec::new();
    let mut assets = Vec::new();
    let mut diagnostics = Vec::new();
    let mut support = SupportLevel::Portable;
    let native_extensions = unsupported_frontmatter_extensions(
        native,
        &frontmatter,
        &["name", "description"],
        &mut diagnostics,
        &mut support,
    );
    for entry in WalkDir::new(root.join(dir))
        .min_depth(1)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() && entry.file_name() != "SKILL.md" {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                asset_paths.push(rel.to_path_buf());
                let relative_path = rel
                    .strip_prefix(dir)
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|_| rel.to_path_buf());
                match fs::read_to_string(entry.path()) {
                    Ok(contents) => assets.push(SkillAsset {
                        relative_path,
                        contents,
                    }),
                    Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
                        support = SupportLevel::Partial;
                        diagnostics.push(Diagnostic {
                            severity: DiagnosticSeverity::Warning,
                            resource_id: Some(format!("skills:{name}")),
                            resource_kind: Some(ResourceKind::Skill),
                            agent: Some(native.agent),
                            message: format!(
                                "skill asset {} is not UTF-8 and cannot be synced in the MVP",
                                rel.display()
                            ),
                        });
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }
    asset_paths.sort();
    assets.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(NormalizedResource {
        id: format!("skills:{name}"),
        kind: ResourceKind::Skill,
        scope: native.scope,
        source_agent: native.agent,
        native_paths: vec![native.path.clone()],
        rule_set: None,
        skill: Some(Skill {
            name,
            description,
            body,
            asset_paths,
            assets,
            frontmatter,
        }),
        subagent: None,
        command: None,
        native_extensions,
        diagnostics,
        support,
    })
}

fn normalize_subagent(
    root: &Path,
    native: &NativeResource,
) -> Result<NormalizedResource, AgentSyncError> {
    let path = root.join(&native.path);
    let raw = fs::read_to_string(&path)?;
    let (frontmatter, body) = split_frontmatter(&raw).map_err(|error| {
        AgentSyncError::Adapter(format!(
            "failed to parse frontmatter in {}: {error}",
            native.path.display()
        ))
    })?;
    let name = frontmatter
        .get("name")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            path.file_stem()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "subagent".to_string());
    let description = frontmatter
        .get("description")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let mut diagnostics = Vec::new();
    let mut support = SupportLevel::Portable;
    if !is_safe_file_stem(&name) {
        support = SupportLevel::Blocked;
        diagnostics.push(subagent_field_diagnostic(
            native,
            "subagent.name: blocked because rendered file names may not contain path separators",
        ));
    }
    let native_extensions = unsupported_subagent_frontmatter_extensions(
        native,
        &frontmatter,
        &mut diagnostics,
        &mut support,
    );
    let tools = normalize_tool_policy(&frontmatter);
    let permissions = frontmatter_object(&frontmatter, &["permissions", "permission"]);
    let model = frontmatter_string(&frontmatter, &["model"]);
    let effort = frontmatter_string(&frontmatter, &["effort"])
        .or_else(|| frontmatter_nested_string(&frontmatter, "config", "effort"));
    let mode = frontmatter_string(&frontmatter, &["mode"]);
    diagnostics.extend(subagent_field_diagnostics(
        native,
        &frontmatter,
        &tools,
        !permissions.is_empty(),
        model.is_some(),
        effort.is_some(),
        mode.is_some(),
    ));
    if subagent_has_blocking_diagnostics(&diagnostics) {
        support = SupportLevel::Blocked;
    } else if native_extensions.contains_key("frontmatter.native") {
        support = SupportLevel::Blocked;
        diagnostics.push(subagent_field_diagnostic(
            native,
            "subagent.native_extensions: blocked until native-only fields can be rendered safely",
        ));
    } else if support == SupportLevel::Portable
        && diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(": partial"))
    {
        support = SupportLevel::Partial;
    }

    Ok(NormalizedResource {
        id: format!("subagents:{name}"),
        kind: ResourceKind::Subagent,
        scope: native.scope,
        source_agent: native.agent,
        native_paths: vec![native.path.clone()],
        rule_set: None,
        skill: None,
        subagent: Some(Subagent {
            name,
            description,
            instructions: body.clone(),
            body,
            model,
            effort,
            tools,
            permissions,
            mode,
            frontmatter,
        }),
        command: None,
        native_extensions,
        diagnostics,
        support,
    })
}

fn normalize_command(
    root: &Path,
    native: &NativeResource,
) -> Result<NormalizedResource, AgentSyncError> {
    let path = root.join(&native.path);
    let raw = fs::read_to_string(&path)?;
    let (frontmatter, template) = split_frontmatter(&raw).map_err(|error| {
        AgentSyncError::Adapter(format!(
            "failed to parse frontmatter in {}: {error}",
            native.path.display()
        ))
    })?;
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "command".to_string());
    command_resource(native, name, template, frontmatter, BTreeMap::new())
}

fn normalize_opencode_config_command(
    root: &Path,
    native: &NativeResource,
) -> Result<NormalizedResource, AgentSyncError> {
    let config = parse_opencode_config(root, native)?;
    let name = native
        .id
        .rsplit(':')
        .next()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "command".to_string());
    let Some(command) = config
        .get("command")
        .and_then(Value::as_object)
        .and_then(|commands| commands.get(&name))
        .cloned()
    else {
        return Ok(blocked_behavior(root, native));
    };
    let mut native_extensions = BTreeMap::new();
    native_extensions.insert("opencode.config".to_string(), config);
    let Some(object) = command.as_object() else {
        let mut resource = command_resource(
            native,
            name,
            String::new(),
            BTreeMap::new(),
            native_extensions,
        )?;
        resource.support = SupportLevel::Blocked;
        resource.diagnostics.push(command_diagnostic(
            native,
            "command.config: blocked because command config entries must be objects",
        ));
        return Ok(resource);
    };
    let template = object
        .get("template")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_default();
    let frontmatter = object
        .iter()
        .filter(|(key, _)| key.as_str() != "template")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    command_resource(native, name, template, frontmatter, native_extensions)
}

fn command_resource(
    native: &NativeResource,
    name: String,
    template: String,
    frontmatter: BTreeMap<String, Value>,
    mut native_extensions: BTreeMap<String, Value>,
) -> Result<NormalizedResource, AgentSyncError> {
    let mut diagnostics = Vec::new();
    let mut support = SupportLevel::Portable;
    if !is_safe_file_stem(&name) {
        support = SupportLevel::Blocked;
        diagnostics.push(command_diagnostic(
            native,
            "command.name: blocked because rendered file names may not contain path separators",
        ));
    }
    if template.trim().is_empty() {
        support = SupportLevel::Blocked;
        diagnostics.push(command_diagnostic(
            native,
            "command.template: blocked because prompt-only commands require a template",
        ));
    }
    if contains_shell_output(&template) {
        support = SupportLevel::Blocked;
        diagnostics.push(command_diagnostic(
            native,
            "command.shell_output: blocked because shell output injection is executable behavior",
        ));
    }
    let command_native_extensions = unsupported_command_frontmatter_extensions(
        native,
        &frontmatter,
        &mut diagnostics,
        &mut support,
    );
    native_extensions.extend(command_native_extensions);
    let description = frontmatter_string(&frontmatter, &["description"]);
    let agent = frontmatter_string(&frontmatter, &["agent"]);
    let model = frontmatter_string(&frontmatter, &["model"]);
    let subtask = frontmatter.get("subtask").and_then(Value::as_bool);
    diagnostics.extend(command_field_diagnostics(
        native,
        agent.is_some(),
        model.is_some(),
        subtask.is_some(),
    ));
    if command_has_blocking_diagnostics(&diagnostics) {
        support = SupportLevel::Blocked;
    } else if native_extensions.contains_key("frontmatter.native") {
        support = SupportLevel::Blocked;
        diagnostics.push(command_diagnostic(
            native,
            "command.native_extensions: blocked until native-only fields can be rendered safely",
        ));
    } else if support == SupportLevel::Portable
        && diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(": partial"))
    {
        support = SupportLevel::Partial;
    }

    Ok(NormalizedResource {
        id: native.id.clone(),
        kind: ResourceKind::Command,
        scope: native.scope,
        source_agent: native.agent,
        native_paths: vec![native.path.clone()],
        rule_set: None,
        skill: None,
        subagent: None,
        command: Some(CommandDefinition {
            name,
            template,
            description,
            agent,
            model,
            subtask,
            frontmatter,
        }),
        native_extensions,
        diagnostics,
        support,
    })
}

fn frontmatter_string(frontmatter: &BTreeMap<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| frontmatter.get(*key).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn frontmatter_nested_string(
    frontmatter: &BTreeMap<String, Value>,
    object_key: &str,
    value_key: &str,
) -> Option<String> {
    frontmatter
        .get(object_key)
        .and_then(Value::as_object)
        .and_then(|object| object.get(value_key))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn frontmatter_object(
    frontmatter: &BTreeMap<String, Value>,
    keys: &[&str],
) -> BTreeMap<String, Value> {
    keys.iter()
        .find_map(|key| frontmatter.get(*key).and_then(Value::as_object))
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn is_safe_file_stem(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\\')
}

fn normalize_tool_policy(frontmatter: &BTreeMap<String, Value>) -> ToolPolicy {
    let Some(value) = frontmatter.get("tools") else {
        return ToolPolicy::default();
    };
    if let Some(items) = value.as_array() {
        return ToolPolicy {
            allow: items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect(),
            deny: Vec::new(),
            native: Some(value.clone()),
        };
    }
    let mut policy = ToolPolicy {
        native: Some(value.clone()),
        ..ToolPolicy::default()
    };
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            match value {
                Value::Bool(true) => policy.allow.push(key.clone()),
                Value::Bool(false) => policy.deny.push(key.clone()),
                Value::Array(items) if key == "allow" => policy.allow.extend(
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToOwned::to_owned),
                ),
                Value::Array(items) if key == "deny" => policy.deny.extend(
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToOwned::to_owned),
                ),
                _ => {}
            }
        }
    }
    policy.allow.sort();
    policy.allow.dedup();
    policy.deny.sort();
    policy.deny.dedup();
    policy
}

fn subagent_field_diagnostics(
    native: &NativeResource,
    frontmatter: &BTreeMap<String, Value>,
    tools: &ToolPolicy,
    has_permissions: bool,
    has_model: bool,
    has_effort: bool,
    has_mode: bool,
) -> Vec<Diagnostic> {
    let mut diagnostics = vec![
        subagent_field_diagnostic(native, "subagent.name: portable"),
        subagent_field_diagnostic(native, "subagent.description: portable"),
        subagent_field_diagnostic(native, "subagent.instructions: portable"),
    ];
    if has_model {
        diagnostics.push(subagent_field_diagnostic(native, "subagent.model: partial"));
    }
    if has_effort {
        diagnostics.push(subagent_field_diagnostic(
            native,
            "subagent.effort: partial",
        ));
    }
    if has_mode {
        diagnostics.push(subagent_field_diagnostic(native, "subagent.mode: partial"));
    }
    if !tools.allow.is_empty() || !tools.deny.is_empty() || tools.native.is_some() {
        diagnostics.push(subagent_field_diagnostic(
            native,
            "subagent.tools: blocked until target-specific tool policy mapping is implemented",
        ));
    }
    if has_permissions {
        diagnostics.push(subagent_field_diagnostic(
            native,
            "subagent.permissions: blocked until permission policy mapping is implemented",
        ));
    }
    for key in ["hooks", "mcp_servers", "mcp", "commands", "plugins"] {
        if frontmatter.contains_key(key) {
            diagnostics.push(subagent_field_diagnostic(
                native,
                &format!("subagent.{key}: blocked behavioral field"),
            ));
        }
    }
    diagnostics
}

fn subagent_field_diagnostic(native: &NativeResource, message: &str) -> Diagnostic {
    Diagnostic {
        severity: if message.contains("blocked") {
            DiagnosticSeverity::Warning
        } else {
            DiagnosticSeverity::Info
        },
        resource_id: Some(native.id.clone()),
        resource_kind: Some(ResourceKind::Subagent),
        agent: Some(native.agent),
        message: message.to_string(),
    }
}

fn subagent_has_blocking_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|diagnostic| {
        diagnostic.resource_kind == Some(ResourceKind::Subagent)
            && diagnostic.severity == DiagnosticSeverity::Warning
            && diagnostic.message.contains("blocked")
    })
}

fn command_field_diagnostics(
    native: &NativeResource,
    has_agent: bool,
    has_model: bool,
    has_subtask: bool,
) -> Vec<Diagnostic> {
    let mut diagnostics = vec![
        command_diagnostic(native, "command.name: portable"),
        command_diagnostic(native, "command.template: portable"),
        command_diagnostic(native, "command.description: portable"),
    ];
    if has_agent {
        diagnostics.push(command_diagnostic(
            native,
            "command.agent: blocked until command execution behavior is explicitly allowed",
        ));
    }
    if has_model {
        diagnostics.push(command_diagnostic(native, "command.model: partial"));
    }
    if has_subtask {
        diagnostics.push(command_diagnostic(
            native,
            "command.subtask: blocked until subagent command execution behavior is explicitly allowed",
        ));
    }
    diagnostics
}

fn command_diagnostic(native: &NativeResource, message: &str) -> Diagnostic {
    Diagnostic {
        severity: if message.contains("blocked") {
            DiagnosticSeverity::Warning
        } else {
            DiagnosticSeverity::Info
        },
        resource_id: Some(native.id.clone()),
        resource_kind: Some(ResourceKind::Command),
        agent: Some(native.agent),
        message: message.to_string(),
    }
}

fn command_has_blocking_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|diagnostic| {
        diagnostic.resource_kind == Some(ResourceKind::Command)
            && diagnostic.severity == DiagnosticSeverity::Warning
            && diagnostic.message.contains("blocked")
    })
}

fn contains_shell_output(template: &str) -> bool {
    template.contains("!`")
}

fn blocked_behavior(root: &Path, native: &NativeResource) -> NormalizedResource {
    let mut native_extensions = BTreeMap::new();
    let mut diagnostics = vec![Diagnostic {
        severity: DiagnosticSeverity::Warning,
        resource_id: Some(native.id.clone()),
        resource_kind: Some(native.kind),
        agent: Some(native.agent),
        message: if native.kind == ResourceKind::Hook
            && matches!(
                native.agent,
                Agent::Codex | Agent::Claude | Agent::CursorCli
            ) {
            "hook resource is partially supported; unsupported entries remain report-only"
                .to_string()
        } else {
            "behavioral resources are blocked for MVP sync".to_string()
        },
    }];
    match fs::read_to_string(root.join(&native.path)) {
        Ok(raw) => {
            add_behavior_diagnostics(native, &raw, &mut native_extensions, &mut diagnostics);
            native_extensions.insert("native.raw".to_string(), Value::String(raw));
        }
        Err(error) => diagnostics.push(Diagnostic {
            severity: DiagnosticSeverity::Warning,
            resource_id: Some(native.id.clone()),
            resource_kind: Some(native.kind),
            agent: Some(native.agent),
            message: format!(
                "could not preserve raw behavioral resource {}: {error}",
                native.path.display()
            ),
        }),
    }
    let support = if native.kind == ResourceKind::Hook
        && matches!(
            native.agent,
            Agent::Codex | Agent::Claude | Agent::CursorCli
        )
        && native_extensions.contains_key("behavior.fields")
    {
        SupportLevel::Partial
    } else {
        SupportLevel::Blocked
    };
    NormalizedResource {
        id: native.id.clone(),
        kind: native.kind,
        scope: native.scope,
        source_agent: native.agent,
        native_paths: vec![native.path.clone()],
        rule_set: None,
        skill: None,
        subagent: None,
        command: None,
        native_extensions,
        diagnostics,
        support,
    }
}

fn add_behavior_diagnostics(
    native: &NativeResource,
    raw: &str,
    native_extensions: &mut BTreeMap<String, Value>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let keys = match (native.agent, native.kind) {
        (Agent::Codex | Agent::Claude | Agent::CursorCli, ResourceKind::Hook) => &["hooks"][..],
        (Agent::Claude, ResourceKind::Permission) => &["permissions"][..],
        (Agent::OpenCode, ResourceKind::Permission) => &["permission"][..],
        _ => &[][..],
    };
    if keys.is_empty() {
        return;
    }
    let Ok(value) = parse_jsonc_value(raw) else {
        diagnostics.push(behavior_diagnostic(
            native,
            "behavior.config: blocked raw behavioral resource could not be parsed for structured diagnostics",
        ));
        return;
    };

    let fields = keys
        .iter()
        .filter_map(|key| {
            value
                .get(*key)
                .map(|field| ((*key).to_string(), field.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    if fields.is_empty() {
        return;
    }

    native_extensions.insert(
        "behavior.kind".to_string(),
        Value::String(native.kind.as_str().to_string()),
    );
    native_extensions.insert(
        "behavior.fields".to_string(),
        Value::Object(fields.clone().into_iter().collect()),
    );
    for (key, field) in fields {
        add_behavior_field_diagnostics(native, &key, &field, diagnostics);
    }
}

fn add_behavior_field_diagnostics(
    native: &NativeResource,
    key: &str,
    field: &Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match native.kind {
        ResourceKind::Hook => {
            diagnostics.push(behavior_diagnostic(
                native,
                &format!("hook.{key}: executable hook behavior requires compatibility mapping"),
            ));
            if let Some(object) = field.as_object() {
                for event in object.keys() {
                    diagnostics.push(behavior_diagnostic(
                        native,
                        &format!(
                            "hook.{event}: executable hook behavior requires compatibility mapping"
                        ),
                    ));
                }
            }
        }
        ResourceKind::Permission => {
            diagnostics.push(behavior_diagnostic(
                native,
                &format!("permission.{key}: blocked permission policy"),
            ));
            if let Some(object) = field.as_object() {
                for policy in object.keys() {
                    diagnostics.push(behavior_diagnostic(
                        native,
                        &format!("permission.{policy}: blocked permission policy"),
                    ));
                }
            }
        }
        _ => {}
    }
}

fn behavior_diagnostic(native: &NativeResource, message: &str) -> Diagnostic {
    Diagnostic {
        severity: DiagnosticSeverity::Warning,
        resource_id: Some(native.id.clone()),
        resource_kind: Some(native.kind),
        agent: Some(native.agent),
        message: message.to_string(),
    }
}

fn unsupported_frontmatter_extensions(
    native: &NativeResource,
    frontmatter: &BTreeMap<String, Value>,
    supported_keys: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
    support: &mut SupportLevel,
) -> BTreeMap<String, Value> {
    let unsupported = frontmatter
        .iter()
        .filter(|(key, _)| !supported_keys.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    if unsupported.is_empty() {
        return BTreeMap::new();
    }

    if *support == SupportLevel::Portable {
        *support = SupportLevel::Partial;
    }
    diagnostics.push(Diagnostic {
        severity: DiagnosticSeverity::Warning,
        resource_id: Some(native.id.clone()),
        resource_kind: Some(native.kind),
        agent: Some(native.agent),
        message: format!(
            "{} frontmatter contains native-only fields that are preserved but not rendered: {}",
            native.kind.as_str(),
            unsupported.keys().cloned().collect::<Vec<_>>().join(", ")
        ),
    });

    let mut native_extensions = BTreeMap::new();
    native_extensions.insert(
        "frontmatter.native".to_string(),
        Value::Object(unsupported.into_iter().collect()),
    );
    native_extensions
}

fn unsupported_subagent_frontmatter_extensions(
    native: &NativeResource,
    frontmatter: &BTreeMap<String, Value>,
    diagnostics: &mut Vec<Diagnostic>,
    support: &mut SupportLevel,
) -> BTreeMap<String, Value> {
    let supported_keys = [
        "name",
        "description",
        "model",
        "effort",
        "tools",
        "permissions",
        "permission",
        "mode",
    ];
    let mut unsupported = BTreeMap::new();
    for (key, value) in frontmatter {
        if key == "config" {
            match value.as_object() {
                Some(config) => {
                    let unsupported_config = config
                        .iter()
                        .filter(|(key, _)| key.as_str() != "effort")
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect::<BTreeMap<_, _>>();
                    if !unsupported_config.is_empty() {
                        unsupported.insert(
                            key.clone(),
                            Value::Object(unsupported_config.into_iter().collect()),
                        );
                    }
                }
                None => {
                    unsupported.insert(key.clone(), value.clone());
                }
            }
            continue;
        }
        if !supported_keys.contains(&key.as_str()) {
            unsupported.insert(key.clone(), value.clone());
        }
    }
    if unsupported.is_empty() {
        return BTreeMap::new();
    }

    if *support == SupportLevel::Portable {
        *support = SupportLevel::Partial;
    }
    diagnostics.push(Diagnostic {
        severity: DiagnosticSeverity::Warning,
        resource_id: Some(native.id.clone()),
        resource_kind: Some(native.kind),
        agent: Some(native.agent),
        message: format!(
            "{} frontmatter contains native-only fields that are preserved but not rendered: {}",
            native.kind.as_str(),
            unsupported.keys().cloned().collect::<Vec<_>>().join(", ")
        ),
    });

    let mut native_extensions = BTreeMap::new();
    native_extensions.insert(
        "frontmatter.native".to_string(),
        Value::Object(unsupported.into_iter().collect()),
    );
    native_extensions
}

fn unsupported_command_frontmatter_extensions(
    native: &NativeResource,
    frontmatter: &BTreeMap<String, Value>,
    diagnostics: &mut Vec<Diagnostic>,
    support: &mut SupportLevel,
) -> BTreeMap<String, Value> {
    let supported_keys = ["description", "agent", "model", "subtask"];
    let unsupported = frontmatter
        .iter()
        .filter(|(key, _)| !supported_keys.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    if unsupported.is_empty() {
        return BTreeMap::new();
    }

    if *support == SupportLevel::Portable {
        *support = SupportLevel::Partial;
    }
    diagnostics.push(Diagnostic {
        severity: DiagnosticSeverity::Warning,
        resource_id: Some(native.id.clone()),
        resource_kind: Some(ResourceKind::Command),
        agent: Some(native.agent),
        message: format!(
            "{} frontmatter contains native-only fields that are preserved but not rendered: {}",
            native.kind.as_str(),
            unsupported.keys().cloned().collect::<Vec<_>>().join(", ")
        ),
    });

    let mut native_extensions = BTreeMap::new();
    native_extensions.insert(
        "frontmatter.native".to_string(),
        Value::Object(unsupported.into_iter().collect()),
    );
    native_extensions
}

fn split_frontmatter(raw: &str) -> Result<(BTreeMap<String, Value>, String), serde_yaml::Error> {
    if let Some(rest) = raw.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            let yaml = &rest[..end];
            let body = rest[end + 5..].to_string();
            let map = if yaml.trim().is_empty() {
                BTreeMap::new()
            } else {
                serde_yaml::from_str::<BTreeMap<String, Value>>(yaml)?
            };
            return Ok((map, body));
        }
    }
    Ok((BTreeMap::new(), raw.to_string()))
}

fn render_native(
    resource: &NormalizedResource,
    target: Agent,
) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
    match resource.kind {
        ResourceKind::RuleSet => render_rules(resource, target),
        ResourceKind::Skill => render_skill(resource, target),
        ResourceKind::Subagent => render_subagent(resource, target),
        ResourceKind::Command => render_command(resource, target),
        ResourceKind::Hook => render_hook(resource, target),
        _ => Err(AgentSyncError::InvalidArgument(
            "only rules, skills, portable subagents, prompt-only commands, and supported hooks can be rendered"
                .to_string(),
        )),
    }
}

fn render_rules(
    resource: &NormalizedResource,
    target: Agent,
) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
    let body = &resource
        .rule_set
        .as_ref()
        .ok_or_else(|| AgentSyncError::Adapter("missing rule body".to_string()))?
        .body;
    let path = match target {
        Agent::Codex | Agent::OpenCode => PathBuf::from("AGENTS.md"),
        Agent::Claude => PathBuf::from("CLAUDE.md"),
        Agent::CursorCli => PathBuf::from(".cursor/rules/agentsync.md"),
    };
    Ok((
        vec![RenderedFile {
            path,
            contents: body.clone(),
        }],
        Vec::new(),
    ))
}

fn render_skill(
    resource: &NormalizedResource,
    target: Agent,
) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
    let skill = resource
        .skill
        .as_ref()
        .ok_or_else(|| AgentSyncError::Adapter("missing skill body".to_string()))?;
    let base = match target {
        Agent::Codex => PathBuf::from(".codex/skills"),
        Agent::Claude => PathBuf::from(".claude/skills"),
        Agent::CursorCli => PathBuf::from(".cursor/skills"),
        Agent::OpenCode => PathBuf::from(".opencode/skills"),
    };
    let skill_dir = base.join(&skill.name);
    let path = skill_dir.join("SKILL.md");
    let mut contents = String::new();
    let portable_frontmatter = skill
        .frontmatter
        .iter()
        .filter(|(key, _)| matches!(key.as_str(), "name" | "description"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    if !portable_frontmatter.is_empty() {
        contents.push_str("---\n");
        for line in serde_yaml::to_string(&portable_frontmatter)
            .map_err(|error| AgentSyncError::Adapter(error.to_string()))?
            .lines()
        {
            if line != "---" {
                contents.push_str(line);
                contents.push('\n');
            }
        }
        contents.push_str("---\n");
    }
    contents.push_str(&skill.body);
    let mut files = vec![RenderedFile { path, contents }];
    for asset in &skill.assets {
        if asset
            .relative_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(AgentSyncError::InvalidArgument(format!(
                "skill asset path may not escape skill directory: {}",
                asset.relative_path.display()
            )));
        }
        files.push(RenderedFile {
            path: skill_dir.join(&asset.relative_path),
            contents: asset.contents.clone(),
        });
    }
    Ok((files, Vec::new()))
}

fn render_subagent(
    resource: &NormalizedResource,
    target: Agent,
) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
    let subagent = resource
        .subagent
        .as_ref()
        .ok_or_else(|| AgentSyncError::Adapter("missing subagent body".to_string()))?;
    match target {
        Agent::Claude => render_markdown_subagent(subagent, PathBuf::from(".claude/agents"), false),
        Agent::OpenCode => {
            render_markdown_subagent(subagent, PathBuf::from(".opencode/agents"), true)
        }
        Agent::Codex => render_codex_subagent(subagent),
        Agent::CursorCli => Err(AgentSyncError::InvalidArgument(
            "cursor subagent rendering is blocked until Cursor publishes stable file-format docs"
                .to_string(),
        )),
    }
}

fn render_markdown_subagent(
    subagent: &Subagent,
    base: PathBuf,
    include_mode: bool,
) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
    let mut frontmatter = BTreeMap::new();
    frontmatter.insert("name".to_string(), Value::String(subagent.name.clone()));
    if let Some(description) = &subagent.description {
        frontmatter.insert(
            "description".to_string(),
            Value::String(description.clone()),
        );
    }
    if let Some(model) = &subagent.model {
        frontmatter.insert("model".to_string(), Value::String(model.clone()));
    }
    if include_mode {
        if let Some(mode) = &subagent.mode {
            frontmatter.insert("mode".to_string(), Value::String(mode.clone()));
        }
    }
    let mut contents = String::new();
    contents.push_str("---\n");
    for line in serde_yaml::to_string(&frontmatter)
        .map_err(|error| AgentSyncError::Adapter(error.to_string()))?
        .lines()
    {
        if line != "---" {
            contents.push_str(line);
            contents.push('\n');
        }
    }
    contents.push_str("---\n");
    contents.push_str(&subagent.instructions);
    Ok((
        vec![RenderedFile {
            path: base.join(format!("{}.md", subagent.name)),
            contents,
        }],
        Vec::new(),
    ))
}

fn render_codex_subagent(
    subagent: &Subagent,
) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
    let mut table = toml::map::Map::new();
    table.insert(
        "name".to_string(),
        toml::Value::String(subagent.name.clone()),
    );
    if let Some(description) = &subagent.description {
        table.insert(
            "description".to_string(),
            toml::Value::String(description.clone()),
        );
    }
    table.insert(
        "instructions".to_string(),
        toml::Value::String(subagent.instructions.clone()),
    );
    if let Some(model) = &subagent.model {
        table.insert("model".to_string(), toml::Value::String(model.clone()));
    }
    if let Some(effort) = &subagent.effort {
        table.insert("effort".to_string(), toml::Value::String(effort.clone()));
    }
    Ok((
        vec![RenderedFile {
            path: PathBuf::from(".codex/agents").join(format!("{}.toml", subagent.name)),
            contents: toml::to_string_pretty(&toml::Value::Table(table))
                .map_err(|error| AgentSyncError::Adapter(error.to_string()))?,
        }],
        Vec::new(),
    ))
}

fn render_command(
    resource: &NormalizedResource,
    target: Agent,
) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
    let command = resource
        .command
        .as_ref()
        .ok_or_else(|| AgentSyncError::Adapter("missing command template".to_string()))?;
    match target {
        Agent::OpenCode => render_opencode_command(command),
        _ => Err(AgentSyncError::InvalidArgument(
            "command rendering is currently only supported for OpenCode targets".to_string(),
        )),
    }
}

fn render_opencode_command(
    command: &CommandDefinition,
) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
    let mut frontmatter = BTreeMap::new();
    if let Some(description) = &command.description {
        frontmatter.insert(
            "description".to_string(),
            Value::String(description.clone()),
        );
    }
    if let Some(agent) = &command.agent {
        frontmatter.insert("agent".to_string(), Value::String(agent.clone()));
    }
    if let Some(model) = &command.model {
        frontmatter.insert("model".to_string(), Value::String(model.clone()));
    }
    if let Some(subtask) = command.subtask {
        frontmatter.insert("subtask".to_string(), Value::Bool(subtask));
    }
    let mut contents = String::new();
    if !frontmatter.is_empty() {
        contents.push_str("---\n");
        for line in serde_yaml::to_string(&frontmatter)
            .map_err(|error| AgentSyncError::Adapter(error.to_string()))?
            .lines()
        {
            if line != "---" {
                contents.push_str(line);
                contents.push('\n');
            }
        }
        contents.push_str("---\n");
    }
    contents.push_str(&command.template);
    Ok((
        vec![RenderedFile {
            path: PathBuf::from(".opencode/commands").join(format!("{}.md", command.name)),
            contents,
        }],
        Vec::new(),
    ))
}

fn render_hook(
    resource: &NormalizedResource,
    target: Agent,
) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
    if !is_direct_hook_agent(resource.source_agent)
        || !matches!(
            target,
            Agent::Codex | Agent::Claude | Agent::CursorCli | Agent::OpenCode
        )
    {
        return Err(AgentSyncError::InvalidArgument(
            "hook rendering is currently supported from Codex, Claude, or Cursor hooks to supported hook targets"
                .to_string(),
        ));
    }

    if target == Agent::OpenCode {
        return render_opencode_hook(resource);
    }

    let (entries, mut diagnostics, mut omitted_entries) = collect_hook_entries(resource)?;
    let mut rendered_handlers = 0usize;
    let mut cursor_hooks = BTreeMap::<String, Vec<Value>>::new();
    let mut nested_hooks = BTreeMap::<String, BTreeMap<Option<String>, Vec<Value>>>::new();

    for entry in entries {
        let Some(target_event) =
            render_hook_event_name(&entry.event, resource.source_agent, target)
        else {
            omitted_entries += 1;
            diagnostics.push(hook_render_diagnostic(
                resource,
                target,
                &format!(
                    "hook.{}: report-only; no direct event mapping to {}",
                    entry.event,
                    target.as_str()
                ),
            ));
            continue;
        };

        let matcher = match entry.matcher.as_deref() {
            Some(matcher) => match render_hook_matcher(matcher, resource.source_agent, target) {
                Ok(matcher) => matcher,
                Err(message) => {
                    omitted_entries += 1;
                    diagnostics.push(hook_render_diagnostic(
                        resource,
                        target,
                        &format!("{}: {message}", entry.location),
                    ));
                    continue;
                }
            },
            None => None,
        };

        let handler = match render_hook_handler(&entry.handler, target) {
            Ok((Some(mut handler), warnings)) => {
                rendered_handlers += 1;
                for warning in warnings {
                    omitted_entries += 1;
                    diagnostics.push(hook_render_diagnostic(
                        resource,
                        target,
                        &format!("{}: {warning}", entry.location),
                    ));
                }
                if target == Agent::CursorCli {
                    if let Some(matcher) = matcher.clone() {
                        if let Some(object) = handler.as_object_mut() {
                            object.insert("matcher".to_string(), Value::String(matcher));
                        }
                    }
                }
                handler
            }
            Ok((None, _)) => {
                omitted_entries += 1;
                diagnostics.push(hook_render_diagnostic(
                    resource,
                    target,
                    &format!("{}: omitted unsupported hook handler", entry.location),
                ));
                continue;
            }
            Err(message) => {
                omitted_entries += 1;
                diagnostics.push(hook_render_diagnostic(
                    resource,
                    target,
                    &format!("{}: {message}", entry.location),
                ));
                continue;
            }
        };

        if target == Agent::CursorCli {
            cursor_hooks.entry(target_event).or_default().push(handler);
        } else {
            nested_hooks
                .entry(target_event)
                .or_default()
                .entry(matcher)
                .or_default()
                .push(handler);
        }
    }

    if cursor_hooks.is_empty() && nested_hooks.is_empty() {
        diagnostics.push(hook_render_diagnostic(
            resource,
            target,
            "hook.render: no hook entries had direct render support",
        ));
        return Ok((Vec::new(), diagnostics));
    }

    diagnostics.push(hook_render_diagnostic(
        resource,
        target,
        &format!(
            "hook.render: rendered {rendered_handlers} command handler(s); omitted {omitted_entries} unsupported hook item(s)"
        ),
    ));

    let contents = if target == Agent::CursorCli {
        render_cursor_hook_contents(cursor_hooks)?
    } else {
        render_nested_hook_contents(nested_hooks)?
    };
    Ok((
        vec![RenderedFile {
            path: hook_target_path(target),
            contents,
        }],
        diagnostics,
    ))
}

#[derive(Debug)]
struct OpenCodeHookEntry {
    event: String,
    tools: Option<Vec<String>>,
    command: String,
    timeout: Option<u64>,
    location: String,
}

fn render_opencode_hook(
    resource: &NormalizedResource,
) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
    let (entries, mut diagnostics, mut omitted_entries) = collect_hook_entries(resource)?;
    let mut rendered_entries = Vec::new();

    for entry in entries {
        let Some(target_event) =
            render_hook_event_name(&entry.event, resource.source_agent, Agent::OpenCode)
        else {
            omitted_entries += 1;
            diagnostics.push(hook_render_diagnostic(
                resource,
                Agent::OpenCode,
                &format!(
                    "hook.{}: report-only; no OpenCode plugin event mapping",
                    entry.event
                ),
            ));
            continue;
        };

        let matcher = match entry.matcher.as_deref() {
            Some(matcher) => {
                match render_hook_matcher(matcher, resource.source_agent, Agent::OpenCode) {
                    Ok(matcher) => matcher,
                    Err(message) => {
                        omitted_entries += 1;
                        diagnostics.push(hook_render_diagnostic(
                            resource,
                            Agent::OpenCode,
                            &format!("{}: {message}", entry.location),
                        ));
                        continue;
                    }
                }
            }
            None => None,
        };

        if !is_opencode_tool_event(&target_event)
            && matcher.as_deref().is_some_and(|m| !m.is_empty())
        {
            omitted_entries += 1;
            diagnostics.push(hook_render_diagnostic(
                resource,
                Agent::OpenCode,
                &format!(
                    "{}: omitted matcher because OpenCode event {target_event} is not a tool hook",
                    entry.location
                ),
            ));
            continue;
        }

        let command = match render_opencode_hook_command(&entry.handler) {
            Ok((Some(command), warnings)) => {
                for warning in warnings {
                    omitted_entries += 1;
                    diagnostics.push(hook_render_diagnostic(
                        resource,
                        Agent::OpenCode,
                        &format!("{}: {warning}", entry.location),
                    ));
                }
                command
            }
            Ok((None, _)) => {
                omitted_entries += 1;
                diagnostics.push(hook_render_diagnostic(
                    resource,
                    Agent::OpenCode,
                    &format!("{}: omitted unsupported hook handler", entry.location),
                ));
                continue;
            }
            Err(message) => {
                omitted_entries += 1;
                diagnostics.push(hook_render_diagnostic(
                    resource,
                    Agent::OpenCode,
                    &format!("{}: {message}", entry.location),
                ));
                continue;
            }
        };

        rendered_entries.push(OpenCodeHookEntry {
            event: target_event,
            tools: matcher.map(|matcher| {
                matcher
                    .split('|')
                    .filter(|tool| !tool.trim().is_empty() && *tool != "*")
                    .map(|tool| tool.trim().to_string())
                    .collect::<Vec<_>>()
            }),
            command: command.command,
            timeout: command.timeout,
            location: entry.location,
        });
    }

    if rendered_entries.is_empty() {
        diagnostics.push(hook_render_diagnostic(
            resource,
            Agent::OpenCode,
            "hook.render: no hook entries had OpenCode plugin render support",
        ));
        return Ok((Vec::new(), diagnostics));
    }

    diagnostics.push(hook_render_diagnostic(
        resource,
        Agent::OpenCode,
        &format!(
            "hook.render: rendered {} command handler(s) to OpenCode plugin shim; omitted {omitted_entries} unsupported hook item(s)",
            rendered_entries.len()
        ),
    ));
    diagnostics.push(hook_render_diagnostic(
        resource,
        Agent::OpenCode,
        "hook.render: OpenCode shim reuses commands, passes adapted JSON on stdin, and maps nonzero exit or decision:block output to plugin errors",
    ));

    Ok((
        vec![RenderedFile {
            path: hook_target_path(Agent::OpenCode),
            contents: render_opencode_hook_plugin(&rendered_entries)?,
        }],
        diagnostics,
    ))
}

#[derive(Debug)]
struct HookEntry {
    event: String,
    matcher: Option<String>,
    handler: Value,
    location: String,
}

fn collect_hook_entries(
    resource: &NormalizedResource,
) -> Result<(Vec<HookEntry>, Vec<Diagnostic>, usize), AgentSyncError> {
    let hooks = resource
        .native_extensions
        .get("behavior.fields")
        .and_then(|fields| fields.get("hooks"))
        .and_then(Value::as_object)
        .ok_or_else(|| AgentSyncError::Adapter("missing hook behavior fields".to_string()))?;

    let mut diagnostics = Vec::new();
    let mut entries = Vec::new();
    let mut omitted_entries = 0usize;
    for (event, groups) in hooks {
        if resource.source_agent == Agent::CursorCli {
            collect_cursor_hook_entries(
                resource,
                event,
                groups,
                &mut entries,
                &mut diagnostics,
                &mut omitted_entries,
            );
        } else {
            collect_nested_hook_entries(
                resource,
                event,
                groups,
                &mut entries,
                &mut diagnostics,
                &mut omitted_entries,
            );
        }
    }
    Ok((entries, diagnostics, omitted_entries))
}

fn collect_cursor_hook_entries(
    resource: &NormalizedResource,
    event: &str,
    handlers: &Value,
    entries: &mut Vec<HookEntry>,
    diagnostics: &mut Vec<Diagnostic>,
    omitted_entries: &mut usize,
) {
    let Some(handlers) = handlers.as_array() else {
        *omitted_entries += 1;
        diagnostics.push(hook_render_diagnostic(
            resource,
            resource.source_agent,
            &format!("hook.{event}: omitted because event handlers are not an array"),
        ));
        return;
    };
    for (handler_index, handler) in handlers.iter().enumerate() {
        let location = format!("hook.{event}[{handler_index}]");
        let Some(object) = handler.as_object() else {
            *omitted_entries += 1;
            diagnostics.push(hook_render_diagnostic(
                resource,
                resource.source_agent,
                &format!("{location}: omitted because handler is not an object"),
            ));
            continue;
        };
        let matcher = match object.get("matcher") {
            Some(matcher) => {
                let Some(matcher) = matcher.as_str() else {
                    *omitted_entries += 1;
                    diagnostics.push(hook_render_diagnostic(
                        resource,
                        resource.source_agent,
                        &format!("{location}: omitted because matcher is not a string"),
                    ));
                    continue;
                };
                Some(matcher.to_string())
            }
            None => None,
        };
        let mut handler = object.clone();
        handler.remove("matcher");
        entries.push(HookEntry {
            event: event.to_string(),
            matcher,
            handler: Value::Object(handler),
            location,
        });
    }
}

fn collect_nested_hook_entries(
    resource: &NormalizedResource,
    event: &str,
    groups: &Value,
    entries: &mut Vec<HookEntry>,
    diagnostics: &mut Vec<Diagnostic>,
    omitted_entries: &mut usize,
) {
    let Some(groups) = groups.as_array() else {
        *omitted_entries += 1;
        diagnostics.push(hook_render_diagnostic(
            resource,
            resource.source_agent,
            &format!("hook.{event}: omitted because event groups are not an array"),
        ));
        return;
    };

    for (group_index, group) in groups.iter().enumerate() {
        let group_location = format!("hook.{event}[{group_index}]");
        let Some(group) = group.as_object() else {
            *omitted_entries += 1;
            diagnostics.push(hook_render_diagnostic(
                resource,
                resource.source_agent,
                &format!("{group_location}: omitted because group is not an object"),
            ));
            continue;
        };
        let unsupported_group_keys = group
            .keys()
            .filter(|key| !matches!(key.as_str(), "matcher" | "hooks"))
            .cloned()
            .collect::<Vec<_>>();
        if !unsupported_group_keys.is_empty() {
            *omitted_entries += 1;
            diagnostics.push(hook_render_diagnostic(
                resource,
                resource.source_agent,
                &format!(
                    "{group_location}: omitted unsupported group fields: {}",
                    unsupported_group_keys.join(", ")
                ),
            ));
            continue;
        }

        let matcher = match group.get("matcher") {
            Some(matcher) => {
                let Some(matcher) = matcher.as_str() else {
                    *omitted_entries += 1;
                    diagnostics.push(hook_render_diagnostic(
                        resource,
                        resource.source_agent,
                        &format!("{group_location}: omitted because matcher is not a string"),
                    ));
                    continue;
                };
                Some(matcher.to_string())
            }
            None => None,
        };
        let Some(handlers) = group.get("hooks").and_then(Value::as_array) else {
            *omitted_entries += 1;
            diagnostics.push(hook_render_diagnostic(
                resource,
                resource.source_agent,
                &format!("{group_location}: omitted because hooks are not an array"),
            ));
            continue;
        };

        for (handler_index, handler) in handlers.iter().enumerate() {
            entries.push(HookEntry {
                event: event.to_string(),
                matcher: matcher.clone(),
                handler: handler.clone(),
                location: format!("{group_location}.hooks[{handler_index}]"),
            });
        }
    }
}

fn render_cursor_hook_contents(
    hooks: BTreeMap<String, Vec<Value>>,
) -> Result<String, AgentSyncError> {
    let mut rendered_hooks = serde_json::Map::new();
    for (event, handlers) in hooks {
        rendered_hooks.insert(event, Value::Array(handlers));
    }
    let mut root = serde_json::Map::new();
    root.insert("version".to_string(), Value::Number(1.into()));
    root.insert("hooks".to_string(), Value::Object(rendered_hooks));
    let contents = format!(
        "{}\n",
        serde_json::to_string_pretty(&Value::Object(root))
            .map_err(|error| AgentSyncError::Adapter(error.to_string()))?
    );
    Ok(contents)
}

fn render_nested_hook_contents(
    hooks: BTreeMap<String, BTreeMap<Option<String>, Vec<Value>>>,
) -> Result<String, AgentSyncError> {
    let mut rendered_hooks = serde_json::Map::new();
    for (event, groups) in hooks {
        let mut rendered_groups = Vec::new();
        for (matcher, handlers) in groups {
            let mut rendered_group = serde_json::Map::new();
            if let Some(matcher) = matcher {
                rendered_group.insert("matcher".to_string(), Value::String(matcher));
            }
            rendered_group.insert("hooks".to_string(), Value::Array(handlers));
            rendered_groups.push(Value::Object(rendered_group));
        }
        rendered_hooks.insert(event, Value::Array(rendered_groups));
    }
    let mut root = serde_json::Map::new();
    root.insert("hooks".to_string(), Value::Object(rendered_hooks));
    let contents = format!(
        "{}\n",
        serde_json::to_string_pretty(&Value::Object(root))
            .map_err(|error| AgentSyncError::Adapter(error.to_string()))?
    );
    Ok(contents)
}

#[derive(Debug)]
struct OpenCodeHookCommand {
    command: String,
    timeout: Option<u64>,
}

fn render_opencode_hook_command(
    handler: &Value,
) -> Result<(Option<OpenCodeHookCommand>, Vec<String>), String> {
    let Some(handler) = handler.as_object() else {
        return Err("omitted because handler is not an object".to_string());
    };
    let allowed = ["type", "command", "timeout", "statusMessage"];
    let unsupported = handler
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>();
    if !unsupported.is_empty() {
        return Err(format!(
            "omitted unsupported handler fields: {}",
            unsupported.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    if let Some(handler_type) = handler.get("type") {
        if handler_type.as_str() != Some("command") {
            return Ok((None, Vec::new()));
        }
    }
    let Some(command) = handler.get("command").and_then(Value::as_str) else {
        return Err("omitted because command handler is missing command".to_string());
    };

    let timeout = match handler.get("timeout") {
        Some(timeout) if timeout.is_u64() => timeout.as_u64(),
        Some(_) => return Err("omitted because timeout is not an unsigned number".to_string()),
        None => None,
    };

    let mut warnings = Vec::new();
    if let Some(status_message) = handler.get("statusMessage") {
        if !status_message.is_string() {
            return Err("omitted because statusMessage is not a string".to_string());
        }
        warnings.push(
            "omitted statusMessage because OpenCode plugin shims do not support it".to_string(),
        );
    }

    Ok((
        Some(OpenCodeHookCommand {
            command: command.to_string(),
            timeout,
        }),
        warnings,
    ))
}

fn render_opencode_hook_plugin(entries: &[OpenCodeHookEntry]) -> Result<String, AgentSyncError> {
    let entries = entries
        .iter()
        .map(|entry| {
            let mut object = serde_json::Map::new();
            object.insert("event".to_string(), Value::String(entry.event.clone()));
            object.insert("command".to_string(), Value::String(entry.command.clone()));
            object.insert(
                "location".to_string(),
                Value::String(entry.location.clone()),
            );
            if let Some(timeout) = entry.timeout {
                object.insert("timeoutSeconds".to_string(), Value::Number(timeout.into()));
            }
            if let Some(tools) = &entry.tools {
                if !tools.is_empty() {
                    object.insert(
                        "tools".to_string(),
                        Value::Array(tools.iter().cloned().map(Value::String).collect()),
                    );
                }
            }
            Value::Object(object)
        })
        .collect::<Vec<_>>();
    let entries = serde_json::to_string_pretty(&Value::Array(entries))
        .map_err(|error| AgentSyncError::Adapter(error.to_string()))?;

    Ok(format!(
        r#"// Generated by AgentSync from Codex/Claude/Cursor hook config.
// OpenCode auto-loads local plugins from .opencode/plugins/.
import {{ spawn }} from "node:child_process";

const AGENTSYNC_HOOKS = {entries};

export const AgentSyncHooks = async (context = {{}}) => {{
  const cwd = context.directory ?? process.cwd();
  return {{
    "tool.execute.before": async (input, output) => {{
      await runAgentSyncHooks("tool.execute.before", {{ input, output }}, input?.tool, cwd);
    }},
    "tool.execute.after": async (input, output) => {{
      await runAgentSyncHooks("tool.execute.after", {{ input, output }}, input?.tool, cwd);
    }},
    "permission.ask": async (input, output) => {{
      await runAgentSyncHooks("permission.ask", {{ input, output }}, undefined, cwd);
    }},
    "experimental.session.compacting": async (input, output) => {{
      await runAgentSyncHooks("experimental.session.compacting", {{ input, output }}, undefined, cwd);
    }},
    event: async (input) => {{
      await runAgentSyncHooks(input?.event?.type, input, undefined, cwd);
    }},
  }};
}};

async function runAgentSyncHooks(eventName, payload, toolName, cwd) {{
  if (!eventName) return;
  for (const entry of AGENTSYNC_HOOKS) {{
    if (entry.event !== eventName) continue;
    if (entry.tools?.length) {{
      if (!toolName || !entry.tools.includes(toolName)) continue;
    }}
    await runCommand(entry, buildHookPayload(eventName, payload, toolName), cwd);
  }}
}}

function buildHookPayload(eventName, payload, toolName) {{
  const input = payload?.input ?? {{}};
  const output = payload?.output ?? {{}};
  const event = payload?.event ?? {{}};
  return {{
    hook_event_name: eventName,
    session_id: input.sessionID ?? event.properties?.sessionID ?? event.properties?.session?.id,
    call_id: input.callID,
    tool_name: toolName ?? input.tool,
    tool_input: output.args ?? input.args,
    tool_output: output.output,
    permission: input,
    opencode: payload,
  }};
}}

async function runCommand(entry, payload, cwd) {{
  return await new Promise((resolve, reject) => {{
    const child = spawn(entry.command, {{
      cwd,
      env: process.env,
      shell: true,
      stdio: ["pipe", "pipe", "pipe"],
    }});
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    const timer = Number.isInteger(entry.timeoutSeconds)
      ? setTimeout(() => {{
          timedOut = true;
          child.kill("SIGTERM");
        }}, entry.timeoutSeconds * 1000)
      : undefined;

    child.stdout.on("data", (chunk) => {{
      stdout += chunk.toString();
    }});
    child.stderr.on("data", (chunk) => {{
      stderr += chunk.toString();
    }});
    child.on("error", (error) => {{
      if (timer) clearTimeout(timer);
      reject(error);
    }});
    child.on("close", (code) => {{
      if (timer) clearTimeout(timer);
      if (timedOut) {{
        reject(new Error(`AgentSync hook ${{entry.location}} timed out after ${{entry.timeoutSeconds}}s`));
        return;
      }}
      if (code !== 0) {{
        reject(new Error(stderr.trim() || stdout.trim() || `AgentSync hook ${{entry.location}} exited with code ${{code}}`));
        return;
      }}
      try {{
        applyHookDecision(entry, stdout, payload);
      }} catch (error) {{
        reject(error);
        return;
      }}
      resolve();
    }});
    child.stdin.end(JSON.stringify(payload));
  }});
}}

function applyHookDecision(entry, stdout, payload) {{
  const text = stdout.trim();
  if (!text) return;
  let message;
  try {{
    message = JSON.parse(text);
  }} catch {{
    return;
  }}
  const decision = message?.decision ?? (message?.block === true ? "block" : undefined);
  if (entry.event === "permission.ask" && payload?.opencode?.output) {{
    if (decision === "block" || decision === "deny") {{
      payload.opencode.output.status = "deny";
      return;
    }}
    if (decision === "allow" || decision === "approve") {{
      payload.opencode.output.status = "allow";
      return;
    }}
  }}
  if (decision === "block" || decision === "deny") {{
    throw new Error(message.reason || message.message || `AgentSync hook ${{entry.location}} blocked execution`);
  }}
}}
"#
    ))
}

fn is_direct_hook_agent(agent: Agent) -> bool {
    matches!(agent, Agent::Codex | Agent::Claude | Agent::CursorCli)
}

fn render_hook_event_name(event: &str, source: Agent, target: Agent) -> Option<String> {
    if source == target {
        return Some(event.to_string());
    }
    match (source, target, event) {
        (Agent::Codex | Agent::Claude, Agent::Codex | Agent::Claude, "PreToolUse")
        | (Agent::Codex | Agent::Claude, Agent::Codex | Agent::Claude, "PermissionRequest")
        | (Agent::Codex | Agent::Claude, Agent::Codex | Agent::Claude, "PostToolUse")
        | (Agent::Codex | Agent::Claude, Agent::Codex | Agent::Claude, "PreCompact")
        | (Agent::Codex | Agent::Claude, Agent::Codex | Agent::Claude, "PostCompact")
        | (Agent::Codex | Agent::Claude, Agent::Codex | Agent::Claude, "SessionStart")
        | (Agent::Codex | Agent::Claude, Agent::Codex | Agent::Claude, "UserPromptSubmit")
        | (Agent::Codex | Agent::Claude, Agent::Codex | Agent::Claude, "SubagentStart")
        | (Agent::Codex | Agent::Claude, Agent::Codex | Agent::Claude, "SubagentStop")
        | (Agent::Codex | Agent::Claude, Agent::Codex | Agent::Claude, "Stop") => {
            Some(event.to_string())
        }
        (Agent::Codex | Agent::Claude, Agent::CursorCli, "PreToolUse") => {
            Some("preToolUse".to_string())
        }
        (Agent::Codex | Agent::Claude, Agent::CursorCli, "PostToolUse") => {
            Some("postToolUse".to_string())
        }
        (Agent::Codex | Agent::Claude, Agent::CursorCli, "SessionStart") => {
            Some("sessionStart".to_string())
        }
        (Agent::Codex | Agent::Claude, Agent::CursorCli, "UserPromptSubmit") => {
            Some("beforeSubmitPrompt".to_string())
        }
        (Agent::Codex | Agent::Claude, Agent::CursorCli, "PreCompact") => {
            Some("preCompact".to_string())
        }
        (Agent::Codex | Agent::Claude, Agent::CursorCli, "SubagentStart") => {
            Some("subagentStart".to_string())
        }
        (Agent::Codex | Agent::Claude, Agent::CursorCli, "SubagentStop") => {
            Some("subagentStop".to_string())
        }
        (Agent::Codex | Agent::Claude, Agent::CursorCli, "Stop") => Some("stop".to_string()),
        (Agent::CursorCli, Agent::Codex | Agent::Claude, "preToolUse") => {
            Some("PreToolUse".to_string())
        }
        (Agent::CursorCli, Agent::Codex | Agent::Claude, "postToolUse") => {
            Some("PostToolUse".to_string())
        }
        (Agent::CursorCli, Agent::Codex | Agent::Claude, "sessionStart") => {
            Some("SessionStart".to_string())
        }
        (Agent::CursorCli, Agent::Codex | Agent::Claude, "beforeSubmitPrompt") => {
            Some("UserPromptSubmit".to_string())
        }
        (Agent::CursorCli, Agent::Codex | Agent::Claude, "preCompact") => {
            Some("PreCompact".to_string())
        }
        (Agent::CursorCli, Agent::Codex | Agent::Claude, "subagentStart") => {
            Some("SubagentStart".to_string())
        }
        (Agent::CursorCli, Agent::Codex | Agent::Claude, "subagentStop") => {
            Some("SubagentStop".to_string())
        }
        (Agent::CursorCli, Agent::Codex | Agent::Claude, "stop") => Some("Stop".to_string()),
        (Agent::Codex | Agent::Claude, Agent::OpenCode, "PreToolUse")
        | (Agent::CursorCli, Agent::OpenCode, "preToolUse") => {
            Some("tool.execute.before".to_string())
        }
        (Agent::Codex | Agent::Claude, Agent::OpenCode, "PostToolUse")
        | (Agent::CursorCli, Agent::OpenCode, "postToolUse") => {
            Some("tool.execute.after".to_string())
        }
        (Agent::Codex | Agent::Claude, Agent::OpenCode, "PermissionRequest") => {
            Some("permission.ask".to_string())
        }
        (Agent::Codex | Agent::Claude, Agent::OpenCode, "SessionStart")
        | (Agent::CursorCli, Agent::OpenCode, "sessionStart") => {
            Some("session.created".to_string())
        }
        (Agent::Codex | Agent::Claude, Agent::OpenCode, "PreCompact")
        | (Agent::CursorCli, Agent::OpenCode, "preCompact") => {
            Some("experimental.session.compacting".to_string())
        }
        (Agent::Codex | Agent::Claude, Agent::OpenCode, "PostCompact") => {
            Some("session.compacted".to_string())
        }
        _ => None,
    }
}

fn is_opencode_tool_event(event: &str) -> bool {
    matches!(event, "tool.execute.before" | "tool.execute.after")
}

fn render_hook_matcher(
    matcher: &str,
    source: Agent,
    target: Agent,
) -> Result<Option<String>, String> {
    if source == target {
        return Ok(Some(matcher.to_string()));
    }
    if matcher.trim().is_empty() {
        return Ok(Some(matcher.to_string()));
    }
    if matcher == "*" {
        return Ok(Some(matcher.to_string()));
    }
    let mut mapped = Vec::new();
    for token in matcher.split('|') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if has_regex_syntax(token) {
            return Err(format!(
                "hook matcher {matcher:?} uses regex syntax that is report-only"
            ));
        }
        let Some(mapped_token) = render_hook_matcher_token(token, source, target) else {
            return Err(format!(
                "hook matcher {matcher:?} contains unsupported tool matcher {token:?}"
            ));
        };
        for part in mapped_token.split('|') {
            if !mapped.iter().any(|existing| existing == part) {
                mapped.push(part.to_string());
            }
        }
    }
    if mapped.is_empty() {
        Err(format!(
            "hook matcher {matcher:?} did not contain supported tool matchers"
        ))
    } else {
        Ok(Some(mapped.join("|")))
    }
}

fn render_hook_matcher_token(token: &str, source: Agent, target: Agent) -> Option<String> {
    match (source, target, token) {
        (Agent::Codex, Agent::Claude, "Bash" | "exec_command") => Some("Bash".to_string()),
        (Agent::Codex, Agent::Claude, "apply_patch") => Some("Write|Edit|MultiEdit".to_string()),
        (Agent::Codex, Agent::Claude, "spawn_agent") => Some("Agent".to_string()),
        (Agent::Claude, Agent::Codex, "Bash") => Some("Bash|exec_command".to_string()),
        (Agent::Claude, Agent::Codex, "Write" | "Edit" | "MultiEdit") => {
            Some("apply_patch|Write|Edit".to_string())
        }
        (Agent::Claude, Agent::Codex, "Agent") => Some("spawn_agent|Agent".to_string()),
        (Agent::Codex | Agent::Claude, Agent::CursorCli, "Bash" | "exec_command") => {
            Some("Shell".to_string())
        }
        (
            Agent::Codex | Agent::Claude,
            Agent::CursorCli,
            "apply_patch" | "Write" | "Edit" | "MultiEdit",
        ) => Some("Write".to_string()),
        (Agent::Codex | Agent::Claude, Agent::CursorCli, "spawn_agent" | "Agent" | "Task") => {
            Some("Task".to_string())
        }
        (Agent::CursorCli, Agent::Codex, "Shell") => Some("Bash|exec_command".to_string()),
        (Agent::CursorCli, Agent::Claude, "Shell") => Some("Bash".to_string()),
        (Agent::CursorCli, Agent::Codex, "Write") => Some("apply_patch|Write|Edit".to_string()),
        (Agent::CursorCli, Agent::Claude, "Write") => Some("Write|Edit|MultiEdit".to_string()),
        (Agent::CursorCli, Agent::Codex, "Task") => Some("spawn_agent|Agent".to_string()),
        (Agent::CursorCli, Agent::Claude, "Task") => Some("Agent".to_string()),
        (Agent::Codex | Agent::Claude | Agent::CursorCli, Agent::OpenCode, "Bash")
        | (Agent::Codex | Agent::Claude | Agent::CursorCli, Agent::OpenCode, "exec_command")
        | (Agent::Codex | Agent::Claude | Agent::CursorCli, Agent::OpenCode, "Shell") => {
            Some("bash".to_string())
        }
        (Agent::Codex | Agent::Claude | Agent::CursorCli, Agent::OpenCode, "Read") => {
            Some("read".to_string())
        }
        (Agent::Codex | Agent::Claude | Agent::CursorCli, Agent::OpenCode, "Grep") => {
            Some("grep".to_string())
        }
        (Agent::Codex | Agent::Claude | Agent::CursorCli, Agent::OpenCode, "Glob") => {
            Some("glob".to_string())
        }
        (
            Agent::Codex | Agent::Claude | Agent::CursorCli,
            Agent::OpenCode,
            "apply_patch" | "Write" | "Edit" | "MultiEdit",
        ) => Some("edit|write|apply_patch".to_string()),
        (Agent::Codex | Agent::Claude | Agent::CursorCli, Agent::OpenCode, "WebFetch") => {
            Some("webfetch".to_string())
        }
        (Agent::Codex | Agent::Claude | Agent::CursorCli, Agent::OpenCode, "WebSearch") => {
            Some("websearch".to_string())
        }
        (_, Agent::CursorCli, "Read" | "Grep") => Some(token.to_string()),
        (Agent::CursorCli, Agent::Codex | Agent::Claude, "Read" | "Grep") => {
            Some(token.to_string())
        }
        (
            Agent::Codex | Agent::Claude,
            Agent::Codex | Agent::Claude,
            "Read" | "Grep" | "Glob" | "WebFetch" | "WebSearch",
        ) => Some(token.to_string()),
        (Agent::Codex | Agent::Claude, Agent::Codex | Agent::Claude, tool)
            if tool.starts_with("mcp__") =>
        {
            Some(tool.to_string())
        }
        (Agent::Codex, Agent::Claude, "Write" | "Edit") => Some(token.to_string()),
        (Agent::Claude, Agent::Codex, "apply_patch" | "exec_command") => Some(token.to_string()),
        _ => None,
    }
}

fn has_regex_syntax(token: &str) -> bool {
    token.chars().any(|char| {
        matches!(
            char,
            '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '+' | '?' | '*' | '\\' | '.'
        )
    })
}

fn render_hook_handler(
    handler: &Value,
    target: Agent,
) -> Result<(Option<Value>, Vec<String>), String> {
    let Some(handler) = handler.as_object() else {
        return Err("omitted because handler is not an object".to_string());
    };
    let allowed = ["type", "command", "timeout", "statusMessage"];
    let unsupported = handler
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>();
    if !unsupported.is_empty() {
        return Err(format!(
            "omitted unsupported handler fields: {}",
            unsupported.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    if let Some(handler_type) = handler.get("type") {
        if handler_type.as_str() != Some("command") {
            return Ok((None, Vec::new()));
        }
    }
    let Some(command) = handler.get("command").and_then(Value::as_str) else {
        return Err("omitted because command handler is missing command".to_string());
    };

    let mut rendered = serde_json::Map::new();
    rendered.insert("type".to_string(), Value::String("command".to_string()));
    rendered.insert("command".to_string(), Value::String(command.to_string()));
    if let Some(timeout) = handler.get("timeout") {
        if !timeout.is_u64() {
            return Err("omitted because timeout is not an unsigned number".to_string());
        }
        rendered.insert("timeout".to_string(), timeout.clone());
    }
    if let Some(status_message) = handler.get("statusMessage") {
        if !status_message.is_string() {
            return Err("omitted because statusMessage is not a string".to_string());
        }
        if target == Agent::CursorCli {
            return Ok((
                Some(Value::Object(rendered)),
                vec![
                    "omitted statusMessage because Cursor hook definitions do not support it"
                        .to_string(),
                ],
            ));
        } else {
            rendered.insert("statusMessage".to_string(), status_message.clone());
        }
    }
    Ok((Some(Value::Object(rendered)), Vec::new()))
}

fn hook_target_path(target: Agent) -> PathBuf {
    match target {
        Agent::Codex => PathBuf::from(".codex/hooks.json"),
        Agent::Claude => PathBuf::from(".claude/settings.json"),
        Agent::CursorCli => PathBuf::from(".cursor/hooks.json"),
        Agent::OpenCode => PathBuf::from(".opencode/plugins/agentsync-hooks.js"),
    }
}

fn hook_render_diagnostic(
    resource: &NormalizedResource,
    target: Agent,
    message: &str,
) -> Diagnostic {
    Diagnostic {
        severity: DiagnosticSeverity::Warning,
        resource_id: Some(resource.id.clone()),
        resource_kind: Some(ResourceKind::Hook),
        agent: Some(target),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn codex_adapter_reports_portable_rules_and_skills() {
        let capabilities = CodexAdapter.capabilities();

        assert_eq!(
            capabilities.resources.get(&ResourceKind::RuleSet),
            Some(&SupportLevel::Portable)
        );
        assert_eq!(
            capabilities.resources.get(&ResourceKind::Skill),
            Some(&SupportLevel::Portable)
        );
        assert_eq!(
            capabilities.resources.get(&ResourceKind::Hook),
            Some(&SupportLevel::Partial)
        );
        assert_eq!(
            capabilities.fields.get("subagent.instructions"),
            Some(&SupportLevel::Portable)
        );
        assert_eq!(
            capabilities.fields.get("subagent.model"),
            Some(&SupportLevel::Partial)
        );
        assert_eq!(
            capabilities.fields.get("subagent.tools"),
            Some(&SupportLevel::Blocked)
        );
        assert_eq!(
            capabilities.resources.get(&ResourceKind::Command),
            Some(&SupportLevel::Partial)
        );
        assert_eq!(
            capabilities.resources.get(&ResourceKind::Plugin),
            Some(&SupportLevel::Blocked)
        );
        assert_eq!(
            capabilities.resources.get(&ResourceKind::Permission),
            Some(&SupportLevel::Blocked)
        );
    }

    #[test]
    fn codex_adapter_discovers_hooks_json_as_partial_behavior() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".codex")).unwrap();
        fs::write(
            dir.path().join(".codex/hooks.json"),
            r#"{"hooks":{"PreToolUse":[{"matcher":"apply_patch","hooks":[{"type":"command","command":"bash .codex/hooks/enforce.sh"}]}],"PostToolUse":[]}}"#,
        )
        .unwrap();

        let resources = CodexAdapter.discover(dir.path(), Scope::Project).unwrap();
        let hook = resources
            .iter()
            .find(|resource| {
                resource.kind == ResourceKind::Hook
                    && resource.path == Path::new(".codex/hooks.json")
            })
            .unwrap();
        let resource = CodexAdapter.read(dir.path(), hook).unwrap();

        assert_eq!(resource.support, SupportLevel::Partial);
        assert_eq!(
            resource.native_extensions["behavior.fields"]["hooks"]["PostToolUse"],
            Value::Array(Vec::new())
        );
        assert!(resource.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("hook.PreToolUse: executable hook behavior requires compatibility mapping")));
    }

    #[test]
    fn claude_adapter_discovers_owned_project_paths() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "rules\n").unwrap();
        fs::create_dir_all(dir.path().join(".claude/skills/review")).unwrap();
        fs::write(
            dir.path().join(".claude/skills/review/SKILL.md"),
            "Review body\n",
        )
        .unwrap();
        fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
        fs::write(dir.path().join(".claude/agents/helper.md"), "agent\n").unwrap();
        fs::create_dir_all(dir.path().join(".claude")).unwrap();
        fs::write(
            dir.path().join(".claude/settings.json"),
            r#"{"hooks":{"PreToolUse":[]}}"#,
        )
        .unwrap();

        let mut resources = ClaudeAdapter.discover(dir.path(), Scope::Project).unwrap();
        resources.sort_by(|a, b| a.path.cmp(&b.path));

        assert_eq!(resources.len(), 4);
        assert!(resources
            .iter()
            .any(|resource| resource.path == Path::new("CLAUDE.md")));
        assert!(resources
            .iter()
            .any(|resource| resource.path == Path::new(".claude/skills/review/SKILL.md")));
        assert!(resources
            .iter()
            .any(|resource| resource.path == Path::new(".claude/agents/helper.md")));
        assert!(resources.iter().any(|resource| {
            resource.kind == ResourceKind::Hook
                && resource.path == Path::new(".claude/settings.json")
        }));
        let hook = resources
            .iter()
            .find(|resource| {
                resource.kind == ResourceKind::Hook
                    && resource.path == Path::new(".claude/settings.json")
            })
            .unwrap();
        let resource = ClaudeAdapter.read(dir.path(), hook).unwrap();
        assert_eq!(
            resource.native_extensions["behavior.fields"]["hooks"]["PreToolUse"],
            Value::Array(Vec::new())
        );
        assert!(resource.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("hook.PreToolUse: executable hook behavior requires compatibility mapping")));
    }

    #[test]
    fn claude_adapter_does_not_treat_regular_settings_as_hooks() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude")).unwrap();
        fs::write(
            dir.path().join(".claude/settings.json"),
            r#"{"permissions":{"allow":[]}}"#,
        )
        .unwrap();

        let resources = ClaudeAdapter.discover(dir.path(), Scope::Project).unwrap();

        assert!(!resources
            .iter()
            .any(|resource| resource.kind == ResourceKind::Hook));
    }

    #[test]
    fn cursor_adapter_discovers_hooks_json_as_partial_behavior() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".cursor")).unwrap();
        fs::write(
            dir.path().join(".cursor/hooks.json"),
            r#"{"version":1,"hooks":{"preToolUse":[{"matcher":"Shell","command":".cursor/hooks/check.sh"}]}}"#,
        )
        .unwrap();

        let resources = CursorCliAdapter
            .discover(dir.path(), Scope::Project)
            .unwrap();
        let hook = resources
            .iter()
            .find(|resource| {
                resource.kind == ResourceKind::Hook
                    && resource.path == Path::new(".cursor/hooks.json")
            })
            .unwrap();
        let resource = CursorCliAdapter.read(dir.path(), hook).unwrap();

        assert_eq!(resource.support, SupportLevel::Partial);
        assert_eq!(
            resource.native_extensions["behavior.fields"]["hooks"]["preToolUse"][0]["matcher"],
            Value::String("Shell".to_string())
        );
        assert!(resource.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("hook.preToolUse: executable hook behavior requires compatibility mapping")));
    }

    #[test]
    fn claude_settings_permissions_are_discovered_as_blocked_behavior() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude")).unwrap();
        fs::write(
            dir.path().join(".claude/settings.json"),
            r#"{"permissions":{"allow":["Bash(git status)"]}}"#,
        )
        .unwrap();

        let resources = ClaudeAdapter.discover(dir.path(), Scope::Project).unwrap();
        let permission = resources
            .iter()
            .find(|resource| {
                resource.kind == ResourceKind::Permission
                    && resource.path == Path::new(".claude/settings.json")
            })
            .unwrap();
        let resource = ClaudeAdapter.read(dir.path(), permission).unwrap();

        assert_eq!(resource.id, "permissions:claude:.claude/settings.json");
        assert_eq!(resource.support, SupportLevel::Blocked);
        assert_eq!(
            resource.native_extensions["behavior.kind"],
            Value::String("permissions".to_string())
        );
        assert_eq!(
            resource.native_extensions["behavior.fields"]["permissions"]["allow"][0],
            Value::String("Bash(git status)".to_string())
        );
        assert!(resource
            .native_extensions
            .get("native.raw")
            .and_then(Value::as_str)
            .unwrap()
            .contains("\"permissions\""));
        assert!(resource.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("permission.allow: blocked permission policy")));
    }

    #[test]
    fn cursor_adapter_renders_rules_to_cursor_rules_file() {
        let resource = NormalizedResource {
            id: "rules:agents-md".to_string(),
            kind: ResourceKind::RuleSet,
            scope: Scope::Project,
            source_agent: Agent::Codex,
            native_paths: vec![PathBuf::from("AGENTS.md")],
            rule_set: Some(RuleSet {
                body: "rules\n".to_string(),
            }),
            skill: None,
            subagent: None,
            command: None,
            native_extensions: BTreeMap::new(),
            diagnostics: Vec::new(),
            support: SupportLevel::Portable,
        };

        let (files, diagnostics) = CursorCliAdapter.render(&resource).unwrap();

        assert!(diagnostics.is_empty());
        assert_eq!(files[0].path, Path::new(".cursor/rules/agentsync.md"));
        assert_eq!(files[0].contents, "rules\n");
    }

    #[test]
    fn skill_frontmatter_preserves_structured_native_fields_without_rendering_them() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude/skills/review")).unwrap();
        fs::write(
            dir.path().join(".claude/skills/review/SKILL.md"),
            r#"---
name: review
description: Review code
tags:
  - rust
  - safety
limits:
  max_files: 3
enabled: true
---
Body
"#,
        )
        .unwrap();
        let native = NativeResource {
            id: "skills:claude:.claude/skills/review/SKILL.md".to_string(),
            agent: Agent::Claude,
            kind: ResourceKind::Skill,
            scope: Scope::Project,
            path: PathBuf::from(".claude/skills/review/SKILL.md"),
        };

        let resource = ClaudeAdapter.read(dir.path(), &native).unwrap();
        let skill = resource.skill.as_ref().unwrap();

        assert_eq!(resource.support, SupportLevel::Partial);
        assert_eq!(
            skill.frontmatter["tags"][0],
            Value::String("rust".to_string())
        );
        assert_eq!(
            skill.frontmatter["limits"]["max_files"],
            Value::Number(3.into())
        );
        assert_eq!(skill.frontmatter["enabled"], Value::Bool(true));
        assert_eq!(
            resource.native_extensions["frontmatter.native"]["tags"][1],
            Value::String("safety".to_string())
        );
        assert!(resource
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("native-only fields")));

        let (files, _) = CodexAdapter.render(&resource).unwrap();

        assert!(files[0].contents.contains("name: review\n"));
        assert!(files[0].contents.contains("description: Review code\n"));
        assert!(!files[0].contents.contains("tags:"));
        assert!(!files[0].contents.contains("limits:"));
        assert!(!files[0].contents.contains("enabled:"));
    }

    #[test]
    fn skill_binary_asset_marks_resource_partial() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude/skills/review/assets")).unwrap();
        fs::write(
            dir.path().join(".claude/skills/review/SKILL.md"),
            "---\nname: review\n---\nBody\n",
        )
        .unwrap();
        fs::write(
            dir.path().join(".claude/skills/review/assets/blob.bin"),
            [0xff, 0xfe, 0xfd],
        )
        .unwrap();
        let native = NativeResource {
            id: "skills:claude:.claude/skills/review/SKILL.md".to_string(),
            agent: Agent::Claude,
            kind: ResourceKind::Skill,
            scope: Scope::Project,
            path: PathBuf::from(".claude/skills/review/SKILL.md"),
        };

        let resource = ClaudeAdapter.read(dir.path(), &native).unwrap();

        assert_eq!(resource.support, SupportLevel::Partial);
        assert!(resource.skill.unwrap().assets.is_empty());
        assert!(resource
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not UTF-8")));
    }

    #[test]
    fn skill_asset_path_traversal_is_rejected_at_render() {
        let mut frontmatter = BTreeMap::new();
        frontmatter.insert("name".to_string(), Value::String("review".to_string()));
        let resource = NormalizedResource {
            id: "skills:review".to_string(),
            kind: ResourceKind::Skill,
            scope: Scope::Project,
            source_agent: Agent::Claude,
            native_paths: vec![PathBuf::from(".claude/skills/review/SKILL.md")],
            rule_set: None,
            skill: Some(Skill {
                name: "review".to_string(),
                description: None,
                body: "Body\n".to_string(),
                asset_paths: vec![PathBuf::from("../escape.md")],
                assets: vec![SkillAsset {
                    relative_path: PathBuf::from("../escape.md"),
                    contents: "escape\n".to_string(),
                }],
                frontmatter,
            }),
            subagent: None,
            command: None,
            native_extensions: BTreeMap::new(),
            diagnostics: Vec::new(),
            support: SupportLevel::Portable,
        };

        let error = CodexAdapter.render(&resource).unwrap_err();

        assert!(error
            .to_string()
            .contains("skill asset path may not escape skill directory"));
    }

    #[test]
    fn opencode_config_reads_literal_instruction_files() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("opencode.json"),
            r#"{"instructions":["docs/rules.md"]}"#,
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join("docs/rules.md"), "project rules\n").unwrap();
        let native = NativeResource {
            id: "rules:opencode:opencode.json".to_string(),
            agent: Agent::OpenCode,
            kind: ResourceKind::RuleSet,
            scope: Scope::Project,
            path: PathBuf::from("opencode.json"),
        };

        let resource = OpenCodeAdapter.read(dir.path(), &native).unwrap();

        assert_eq!(resource.support, SupportLevel::Portable);
        assert!(resource.diagnostics.is_empty());
        assert_eq!(
            resource.rule_set.unwrap().body,
            "<!-- docs/rules.md -->\nproject rules\n"
        );
        assert_eq!(
            resource.native_paths,
            vec![
                PathBuf::from("docs/rules.md"),
                PathBuf::from("opencode.json")
            ]
        );
        assert!(resource.native_extensions.contains_key("opencode.config"));
    }

    #[test]
    fn opencode_jsonc_config_reads_instructions_and_behavior_keys() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("opencode.jsonc"),
            r#"{
  // project config
  "instructions": ["docs/rules.md"],
  "agent": {
    "code-reviewer": {
      "description": "Reviews code",
      "tools": {
        "write": false,
      },
    },
  },
  "command": {
    "deploy": {
      "template": "Deploy the app",
    },
  },
  "plugin": ["opencode-wakatime"],
}"#,
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join("docs/rules.md"), "project rules\n").unwrap();

        let resources = OpenCodeAdapter
            .discover(dir.path(), Scope::Project)
            .unwrap();

        assert!(resources.iter().any(|resource| {
            resource.kind == ResourceKind::RuleSet && resource.path == Path::new("opencode.jsonc")
        }));
        assert!(resources.iter().any(|resource| {
            resource.kind == ResourceKind::Subagent && resource.path == Path::new("opencode.jsonc")
        }));
        assert!(resources.iter().any(|resource| {
            resource.kind == ResourceKind::Command && resource.path == Path::new("opencode.jsonc")
        }));
        assert!(resources.iter().any(|resource| {
            resource.kind == ResourceKind::Plugin && resource.path == Path::new("opencode.jsonc")
        }));

        let native = NativeResource {
            id: "rules:opencode:opencode.jsonc".to_string(),
            agent: Agent::OpenCode,
            kind: ResourceKind::RuleSet,
            scope: Scope::Project,
            path: PathBuf::from("opencode.jsonc"),
        };
        let resource = OpenCodeAdapter.read(dir.path(), &native).unwrap();

        assert_eq!(
            resource.rule_set.unwrap().body,
            "<!-- docs/rules.md -->\nproject rules\n"
        );
    }

    #[test]
    fn opencode_config_agents_are_discovered_as_blocked_behavior() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("opencode.json"),
            r#"{"agent":{"code-reviewer":{"description":"Reviews code","tools":{"write":false}}}}"#,
        )
        .unwrap();

        let resources = OpenCodeAdapter
            .discover(dir.path(), Scope::Project)
            .unwrap();
        let agent = resources
            .iter()
            .find(|resource| {
                resource.kind == ResourceKind::Subagent
                    && resource.path == Path::new("opencode.json")
            })
            .unwrap();
        let resource = OpenCodeAdapter.read(dir.path(), agent).unwrap();

        assert_eq!(resource.id, "subagents:opencode:opencode.json");
        assert_eq!(resource.support, SupportLevel::Blocked);
        assert!(resource.subagent.is_none());
        assert!(resource
            .native_extensions
            .get("native.raw")
            .and_then(Value::as_str)
            .unwrap()
            .contains("\"agent\""));
    }

    #[test]
    fn opencode_permissions_are_discovered_as_blocked_behavior() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("opencode.jsonc"),
            r#"{
  "permission": {
    "edit": "ask",
    "bash": {
      "git status": "allow",
    },
  },
}"#,
        )
        .unwrap();

        let resources = OpenCodeAdapter
            .discover(dir.path(), Scope::Project)
            .unwrap();
        let permission = resources
            .iter()
            .find(|resource| {
                resource.kind == ResourceKind::Permission
                    && resource.path == Path::new("opencode.jsonc")
            })
            .unwrap();
        let resource = OpenCodeAdapter.read(dir.path(), permission).unwrap();

        assert_eq!(resource.id, "permissions:opencode:opencode.jsonc");
        assert_eq!(resource.support, SupportLevel::Blocked);
        assert_eq!(
            resource.native_extensions["behavior.kind"],
            Value::String("permissions".to_string())
        );
        assert_eq!(
            resource.native_extensions["behavior.fields"]["permission"]["edit"],
            Value::String("ask".to_string())
        );
        assert!(resource
            .native_extensions
            .get("native.raw")
            .and_then(Value::as_str)
            .unwrap()
            .contains("\"permission\""));
        assert!(resource.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("permission.bash: blocked permission policy")));
    }

    #[test]
    fn opencode_jsonc_rejects_loose_non_jsonc_syntax() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("opencode.jsonc"),
            r#"{
  instructions: ["docs/rules.md"],
}"#,
        )
        .unwrap();
        let native = NativeResource {
            id: "rules:opencode:opencode.jsonc".to_string(),
            agent: Agent::OpenCode,
            kind: ResourceKind::RuleSet,
            scope: Scope::Project,
            path: PathBuf::from("opencode.jsonc"),
        };

        assert!(OpenCodeAdapter
            .read(dir.path(), &native)
            .unwrap_err()
            .to_string()
            .contains("failed to parse opencode.jsonc as JSONC"));
    }

    #[test]
    fn opencode_config_marks_glob_instruction_partial() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("opencode.json"),
            r#"{"instructions":["docs/*.md"]}"#,
        )
        .unwrap();
        let native = NativeResource {
            id: "rules:opencode:opencode.json".to_string(),
            agent: Agent::OpenCode,
            kind: ResourceKind::RuleSet,
            scope: Scope::Project,
            path: PathBuf::from("opencode.json"),
        };

        let resource = OpenCodeAdapter.read(dir.path(), &native).unwrap();

        assert_eq!(resource.support, SupportLevel::Partial);
        assert!(resource.rule_set.unwrap().body.is_empty());
        assert!(resource
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("uses a glob")));
    }

    #[test]
    fn opencode_config_command_is_normalized_as_prompt_data() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("opencode.json"),
            r#"{"command":{"deploy":{"template":"Deploy the app"}}}"#,
        )
        .unwrap();

        let resources = OpenCodeAdapter
            .discover(dir.path(), Scope::Project)
            .unwrap();
        let command = resources
            .iter()
            .find(|resource| {
                resource.kind == ResourceKind::Command
                    && resource.path == Path::new("opencode.json")
            })
            .unwrap();
        let resource = OpenCodeAdapter.read(dir.path(), command).unwrap();

        assert_eq!(resource.id, "commands:opencode:opencode.json:deploy");
        assert_eq!(resource.support, SupportLevel::Portable);
        let command = resource.command.as_ref().unwrap();
        assert_eq!(command.name, "deploy");
        assert_eq!(command.template, "Deploy the app");
        assert!(resource
            .native_extensions
            .get("opencode.config")
            .and_then(|value| value.get("command"))
            .is_some());
    }

    #[test]
    fn opencode_plugins_are_discovered_as_blocked_behavior() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".opencode/plugins")).unwrap();
        fs::write(
            dir.path().join(".opencode/plugins/notify.ts"),
            "export const Notify = async () => ({})\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("opencode.json"),
            r#"{"plugin":["opencode-wakatime"]}"#,
        )
        .unwrap();

        let mut resources = OpenCodeAdapter
            .discover(dir.path(), Scope::Project)
            .unwrap();
        resources.sort_by(|a, b| a.id.cmp(&b.id));

        assert!(resources.iter().any(|resource| {
            resource.kind == ResourceKind::Plugin
                && resource.path == Path::new(".opencode/plugins/notify.ts")
        }));
        let config_plugin = resources
            .iter()
            .find(|resource| {
                resource.kind == ResourceKind::Plugin && resource.path == Path::new("opencode.json")
            })
            .unwrap();
        let resource = OpenCodeAdapter.read(dir.path(), config_plugin).unwrap();

        assert_eq!(resource.id, "plugins:opencode:opencode.json");
        assert_eq!(resource.support, SupportLevel::Blocked);
        assert!(resource
            .native_extensions
            .get("native.raw")
            .and_then(Value::as_str)
            .unwrap()
            .contains("\"plugin\""));
    }

    #[test]
    fn opencode_generated_hook_plugin_is_not_rediscovered_as_native_plugin() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".opencode/plugins")).unwrap();
        fs::write(
            dir.path().join(".opencode/plugins/agentsync-hooks.js"),
            "// Generated by AgentSync from Codex/Claude/Cursor hook config.\nexport const AgentSyncHooks = async () => ({});\n",
        )
        .unwrap();

        let resources = OpenCodeAdapter
            .discover(dir.path(), Scope::Project)
            .unwrap();

        assert!(!resources.iter().any(|resource| {
            resource.kind == ResourceKind::Plugin
                && resource.path == Path::new(".opencode/plugins/agentsync-hooks.js")
        }));
    }

    #[test]
    fn opencode_user_plugin_named_agentsync_hooks_is_still_discovered() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".opencode/plugins")).unwrap();
        fs::write(
            dir.path().join(".opencode/plugins/agentsync-hooks.js"),
            "export const CustomPlugin = async () => ({});\n",
        )
        .unwrap();

        let resources = OpenCodeAdapter
            .discover(dir.path(), Scope::Project)
            .unwrap();

        assert!(resources.iter().any(|resource| {
            resource.kind == ResourceKind::Plugin
                && resource.path == Path::new(".opencode/plugins/agentsync-hooks.js")
        }));
    }

    #[test]
    fn claude_subagent_is_normalized_read_only() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
        fs::write(
            dir.path().join(".claude/agents/reviewer.md"),
            r#"---
name: reviewer
description: Review code
model: sonnet
tools:
  - Read
  - Grep
permissions:
  edit: allow
config:
  effort: high
enabled: true
---
Review carefully.
"#,
        )
        .unwrap();
        let native = NativeResource {
            id: "subagents:claude:.claude/agents/reviewer.md".to_string(),
            agent: Agent::Claude,
            kind: ResourceKind::Subagent,
            scope: Scope::Project,
            path: PathBuf::from(".claude/agents/reviewer.md"),
        };

        let resource = ClaudeAdapter.read(dir.path(), &native).unwrap();
        let subagent = resource.subagent.unwrap();

        assert_eq!(resource.id, "subagents:reviewer");
        assert_eq!(resource.support, SupportLevel::Blocked);
        assert_eq!(subagent.name, "reviewer");
        assert_eq!(subagent.description.as_deref(), Some("Review code"));
        assert_eq!(subagent.body, "Review carefully.\n");
        assert_eq!(subagent.instructions, "Review carefully.\n");
        assert_eq!(subagent.model.as_deref(), Some("sonnet"));
        assert_eq!(subagent.effort.as_deref(), Some("high"));
        assert_eq!(subagent.tools.allow, vec!["Read", "Grep"]);
        assert!(subagent.tools.deny.is_empty());
        assert_eq!(
            subagent.permissions["edit"],
            Value::String("allow".to_string())
        );
        assert_eq!(
            subagent.frontmatter["tools"][1],
            Value::String("Grep".to_string())
        );
        assert_eq!(subagent.frontmatter["enabled"], Value::Bool(true));
        assert_eq!(
            resource.native_extensions["frontmatter.native"]["enabled"],
            Value::Bool(true)
        );
        assert!(resource
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "subagent.model: partial"));
        assert!(resource.diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "subagent.model: partial"
                && diagnostic.severity == DiagnosticSeverity::Info
        }));
        assert!(resource
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("subagent.tools: blocked")));
        assert!(resource
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("subagent.permissions: blocked")));
    }

    #[test]
    fn subagent_tool_object_normalizes_allow_deny_and_mode() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".opencode/agents")).unwrap();
        fs::write(
            dir.path().join(".opencode/agents/operator.md"),
            r#"---
name: operator
description: Operate carefully
mode: primary
tools:
  read: true
  write: false
---
Operate carefully.
"#,
        )
        .unwrap();
        let native = NativeResource {
            id: "subagents:opencode:.opencode/agents/operator.md".to_string(),
            agent: Agent::OpenCode,
            kind: ResourceKind::Subagent,
            scope: Scope::Project,
            path: PathBuf::from(".opencode/agents/operator.md"),
        };

        let resource = OpenCodeAdapter.read(dir.path(), &native).unwrap();
        let subagent = resource.subagent.unwrap();

        assert_eq!(subagent.mode.as_deref(), Some("primary"));
        assert_eq!(subagent.tools.allow, vec!["read"]);
        assert_eq!(subagent.tools.deny, vec!["write"]);
        assert!(resource
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "subagent.mode: partial"));
        assert!(resource
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("subagent.tools: blocked")));
    }

    #[test]
    fn opencode_subagent_name_falls_back_to_file_stem() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".opencode/agents")).unwrap();
        fs::write(
            dir.path().join(".opencode/agents/planner.md"),
            "Plan the work.\n",
        )
        .unwrap();
        let native = NativeResource {
            id: "subagents:opencode:.opencode/agents/planner.md".to_string(),
            agent: Agent::OpenCode,
            kind: ResourceKind::Subagent,
            scope: Scope::Project,
            path: PathBuf::from(".opencode/agents/planner.md"),
        };

        let resource = OpenCodeAdapter.read(dir.path(), &native).unwrap();

        assert_eq!(resource.id, "subagents:planner");
        assert_eq!(resource.subagent.unwrap().name, "planner");
    }
}
