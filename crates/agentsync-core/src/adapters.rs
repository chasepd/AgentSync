use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use walkdir::WalkDir;

use crate::diagnostics::{AgentSyncError, Diagnostic, DiagnosticSeverity};
use crate::model::{
    AdapterCapabilities, Agent, NativeResource, NormalizedResource, RenderedFile, ResourceKind,
    RuleSet, Scope, Skill, SupportLevel,
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
        Box::new(CursorAdapter),
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
        );
        discover_skill_dirs(
            &mut resources,
            root,
            self.agent(),
            [".codex/skills", ".agents/skills"],
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
        if scope != Scope::Project {
            return Ok(Vec::new());
        }
        let mut resources = Vec::new();
        push_if_exists(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::RuleSet,
            "CLAUDE.md",
        );
        push_if_exists(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::RuleSet,
            ".claude/CLAUDE.md",
        );
        discover_skill_dirs(&mut resources, root, self.agent(), [".claude/skills"]);
        discover_md_dir(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Subagent,
            ".claude/agents",
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
pub struct CursorAdapter;

impl AgentAdapter for CursorAdapter {
    fn agent(&self) -> Agent {
        Agent::Cursor
    }

    fn capabilities(&self) -> AdapterCapabilities {
        portable_capabilities(Agent::Cursor)
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
        );
        discover_md_dir(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::RuleSet,
            ".cursor/rules",
        );
        discover_skill_dirs(&mut resources, root, self.agent(), [".cursor/skills"]);
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
        render_native(resource, Agent::Cursor)
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
        );
        push_if_exists(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::RuleSet,
            "opencode.json",
        );
        discover_skill_dirs(&mut resources, root, self.agent(), [".opencode/skills"]);
        discover_md_dir(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Subagent,
            ".opencode/agents",
        );
        discover_md_dir(
            &mut resources,
            root,
            self.agent(),
            ResourceKind::Command,
            ".opencode/commands",
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

fn portable_capabilities(agent: Agent) -> AdapterCapabilities {
    let mut resources = BTreeMap::new();
    resources.insert(ResourceKind::RuleSet, SupportLevel::Portable);
    resources.insert(ResourceKind::Skill, SupportLevel::Portable);
    resources.insert(ResourceKind::Subagent, SupportLevel::Blocked);
    resources.insert(ResourceKind::Hook, SupportLevel::Blocked);
    resources.insert(ResourceKind::Command, SupportLevel::Blocked);

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
) {
    if root.join(rel).is_file() {
        resources.push(native(agent, kind, rel));
    }
}

fn discover_skill_dirs<const N: usize>(
    resources: &mut Vec<NativeResource>,
    root: &Path,
    agent: Agent,
    dirs: [&str; N],
) {
    for dir in dirs {
        if let Ok(entries) = fs::read_dir(root.join(dir)) {
            for entry in entries.flatten() {
                let skill = entry.path().join("SKILL.md");
                if skill.is_file() {
                    if let Ok(rel) = skill.strip_prefix(root) {
                        resources.push(native(agent, ResourceKind::Skill, &rel.to_string_lossy()));
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
                    resources.push(native(agent, kind, &rel.to_string_lossy()));
                }
            }
        }
    }
}

fn native(agent: Agent, kind: ResourceKind, rel: &str) -> NativeResource {
    NativeResource {
        id: format!(
            "{}:{}:{}",
            kind.as_str(),
            agent.as_str(),
            rel.replace('\\', "/")
        ),
        agent,
        kind,
        scope: Scope::Project,
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
        ResourceKind::Subagent | ResourceKind::Hook | ResourceKind::Command => {
            Ok(blocked_behavior(native))
        }
    }
}

fn normalize_rule(
    root: &Path,
    native: &NativeResource,
) -> Result<NormalizedResource, AgentSyncError> {
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
        native_extensions: BTreeMap::new(),
        diagnostics: Vec::new(),
        support: SupportLevel::Portable,
    })
}

fn normalize_skill(
    root: &Path,
    native: &NativeResource,
) -> Result<NormalizedResource, AgentSyncError> {
    let path = root.join(&native.path);
    let raw = fs::read_to_string(&path)?;
    let (frontmatter, body) = split_frontmatter(&raw);
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
    for entry in WalkDir::new(root.join(dir))
        .min_depth(1)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() && entry.file_name() != "SKILL.md" {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                asset_paths.push(rel.to_path_buf());
            }
        }
    }
    asset_paths.sort();
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
            frontmatter,
        }),
        native_extensions: BTreeMap::new(),
        diagnostics: Vec::new(),
        support: SupportLevel::Portable,
    })
}

fn blocked_behavior(native: &NativeResource) -> NormalizedResource {
    NormalizedResource {
        id: native.id.clone(),
        kind: native.kind,
        scope: native.scope,
        source_agent: native.agent,
        native_paths: vec![native.path.clone()],
        rule_set: None,
        skill: None,
        native_extensions: BTreeMap::new(),
        diagnostics: vec![Diagnostic {
            severity: DiagnosticSeverity::Warning,
            resource_id: Some(native.id.clone()),
            resource_kind: Some(native.kind),
            agent: Some(native.agent),
            message: "behavioral resources are blocked for MVP sync".to_string(),
        }],
        support: SupportLevel::Blocked,
    }
}

fn split_frontmatter(raw: &str) -> (BTreeMap<String, Value>, String) {
    if let Some(rest) = raw.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            let yaml_like = &rest[..end];
            let body = rest[end + 5..].to_string();
            let mut map = BTreeMap::new();
            for line in yaml_like.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    map.insert(
                        key.trim().to_string(),
                        Value::String(value.trim().trim_matches('"').to_string()),
                    );
                }
            }
            return (map, body);
        }
    }
    (BTreeMap::new(), raw.to_string())
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
        Agent::Cursor => PathBuf::from(".cursor/rules/agentsync.md"),
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
        Agent::Cursor => PathBuf::from(".cursor/skills"),
        Agent::OpenCode => PathBuf::from(".opencode/skills"),
    };
    let path = base.join(&skill.name).join("SKILL.md");
    let mut contents = String::new();
    if !skill.frontmatter.is_empty() {
        contents.push_str("---\n");
        for (key, value) in &skill.frontmatter {
            let value = value.as_str().unwrap_or_default();
            contents.push_str(&format!("{key}: {value}\n"));
        }
        contents.push_str("---\n");
    }
    contents.push_str(&skill.body);
    Ok((vec![RenderedFile { path, contents }], Vec::new()))
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

        let mut resources = ClaudeAdapter.discover(dir.path(), Scope::Project).unwrap();
        resources.sort_by(|a, b| a.path.cmp(&b.path));

        assert_eq!(resources.len(), 3);
        assert!(resources
            .iter()
            .any(|resource| resource.path == Path::new("CLAUDE.md")));
        assert!(resources
            .iter()
            .any(|resource| resource.path == Path::new(".claude/skills/review/SKILL.md")));
        assert!(resources
            .iter()
            .any(|resource| resource.path == Path::new(".claude/agents/helper.md")));
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
            native_extensions: BTreeMap::new(),
            diagnostics: Vec::new(),
            support: SupportLevel::Portable,
        };

        let (files, diagnostics) = CursorAdapter.render(&resource).unwrap();

        assert!(diagnostics.is_empty());
        assert_eq!(files[0].path, Path::new(".cursor/rules/agentsync.md"));
        assert_eq!(files[0].contents, "rules\n");
    }
}
