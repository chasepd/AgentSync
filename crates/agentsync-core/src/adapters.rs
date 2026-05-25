use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use jsonc_parser::{errors::ParseError, parse_to_serde_value, ParseOptions};
use serde_json::Value;
use walkdir::WalkDir;

use crate::diagnostics::{AgentSyncError, Diagnostic, DiagnosticSeverity};
use crate::model::{
    AdapterCapabilities, Agent, NativeResource, NormalizedResource, RenderedFile, ResourceKind,
    RuleSet, Scope, Skill, SkillAsset, Subagent, SupportLevel,
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
        let (rules, skills) = match scope {
            Scope::Project => ("AGENTS.md", [".codex/skills", ".agents/skills"]),
            Scope::User => (".codex/AGENTS.md", [".codex/skills", ".agents/skills"]),
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
        discover_json_key_files(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Command,
            config_files,
            "command",
            scope,
        );
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
    resources.insert(ResourceKind::Subagent, SupportLevel::Blocked);
    resources.insert(ResourceKind::Hook, SupportLevel::Blocked);
    resources.insert(ResourceKind::Command, SupportLevel::Blocked);
    resources.insert(ResourceKind::Plugin, SupportLevel::Blocked);
    resources.insert(ResourceKind::Permission, SupportLevel::Blocked);

    let mut fields = BTreeMap::new();
    fields.insert("rules.body".to_string(), SupportLevel::Portable);
    fields.insert("skill.name".to_string(), SupportLevel::Portable);
    fields.insert("skill.description".to_string(), SupportLevel::Portable);
    fields.insert("skill.body".to_string(), SupportLevel::Portable);
    fields.insert("skill.assets".to_string(), SupportLevel::Partial);
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
                    resources.push(native(agent, kind, &rel.to_string_lossy(), scope));
                }
            }
        }
    }
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
    let mut diagnostics = vec![Diagnostic {
        severity: DiagnosticSeverity::Warning,
        resource_id: Some(native.id.clone()),
        resource_kind: Some(ResourceKind::Subagent),
        agent: Some(native.agent),
        message: "subagent rendering is blocked for MVP sync".to_string(),
    }];
    let mut support = SupportLevel::Blocked;
    let native_extensions = unsupported_frontmatter_extensions(
        native,
        &frontmatter,
        &["name", "description"],
        &mut diagnostics,
        &mut support,
    );

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
            body,
            frontmatter,
        }),
        native_extensions,
        diagnostics,
        support,
    })
}

fn blocked_behavior(root: &Path, native: &NativeResource) -> NormalizedResource {
    let mut native_extensions = BTreeMap::new();
    let mut diagnostics = vec![Diagnostic {
        severity: DiagnosticSeverity::Warning,
        resource_id: Some(native.id.clone()),
        resource_kind: Some(native.kind),
        agent: Some(native.agent),
        message: "behavioral resources are blocked for MVP sync".to_string(),
    }];
    match fs::read_to_string(root.join(&native.path)) {
        Ok(raw) => {
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
    NormalizedResource {
        id: native.id.clone(),
        kind: native.kind,
        scope: native.scope,
        source_agent: native.agent,
        native_paths: vec![native.path.clone()],
        rule_set: None,
        skill: None,
        subagent: None,
        native_extensions,
        diagnostics,
        support: SupportLevel::Blocked,
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
        _ => Err(AgentSyncError::InvalidArgument(
            "only rules and skills can be rendered in the MVP".to_string(),
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
            capabilities.resources.get(&ResourceKind::Command),
            Some(&SupportLevel::Blocked)
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
        assert!(resource
            .native_extensions
            .get("native.raw")
            .and_then(Value::as_str)
            .unwrap()
            .contains("\"permissions\""));
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
        assert!(resource
            .native_extensions
            .get("native.raw")
            .and_then(Value::as_str)
            .unwrap()
            .contains("\"permission\""));
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
    fn opencode_config_command_is_discovered_as_blocked_behavior() {
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

        assert_eq!(resource.id, "commands:opencode:opencode.json");
        assert_eq!(resource.support, SupportLevel::Blocked);
        assert!(resource
            .native_extensions
            .get("native.raw")
            .and_then(Value::as_str)
            .unwrap()
            .contains("\"command\""));
        assert!(resource.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("behavioral resources are blocked")));
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
        assert_eq!(
            subagent.frontmatter.get("model").and_then(Value::as_str),
            Some("sonnet")
        );
        assert_eq!(
            subagent.frontmatter["tools"][1],
            Value::String("Grep".to_string())
        );
        assert_eq!(
            subagent.frontmatter["config"]["effort"],
            Value::String("high".to_string())
        );
        assert_eq!(subagent.frontmatter["enabled"], Value::Bool(true));
        assert_eq!(
            resource.native_extensions["frontmatter.native"]["tools"][0],
            Value::String("Read".to_string())
        );
        assert!(resource
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("subagent rendering is blocked")));
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
