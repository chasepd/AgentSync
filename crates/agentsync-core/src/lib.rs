pub mod adapters;
pub mod diagnostics;
pub mod model;
pub mod report;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use model::{NativeResource, NormalizedResource, RenderedFile, RuleSet, Skill, SupportLevel};
use report::{
    DriftState, PlanAction, PlanActionKind, PlanReport, ScanReport, StatusItem, StatusReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

pub use diagnostics::{AgentSyncError, Diagnostic, DiagnosticSeverity};
pub use model::{Agent, ResourceKind, ResourceSelector, Scope, SourceAlias};

pub fn scan(scope: Scope) -> Result<ScanReport, AgentSyncError> {
    scan_root(std::env::current_dir()?, scope)
}

pub fn scan_root(root: impl AsRef<Path>, scope: Scope) -> Result<ScanReport, AgentSyncError> {
    let root = root.as_ref();
    let mut report = ScanReport::empty(scope);
    if matches!(scope, Scope::User | Scope::All) {
        report.diagnostics.push(Diagnostic {
            severity: DiagnosticSeverity::Info,
            resource_id: None,
            resource_kind: None,
            agent: None,
            message: "user scope scanning is not implemented yet".to_string(),
        });
    }
    if matches!(scope, Scope::Project | Scope::All) {
        report.resources = discover_project(root);
        for native in &report.resources {
            match normalize(root, native) {
                Ok(resource) => report.normalized.push(resource),
                Err(error) => report.diagnostics.push(Diagnostic {
                    severity: DiagnosticSeverity::Warning,
                    resource_id: Some(native.id.clone()),
                    resource_kind: Some(native.kind),
                    agent: Some(native.agent),
                    message: error.to_string(),
                }),
            }
        }
    }
    report.resources.sort_by_key(resource_sort_key);
    report.normalized.sort_by(|a, b| a.id.cmp(&b.id));
    report.normalized.dedup_by(|a, b| a.id == b.id);
    Ok(report)
}

pub fn status(scope: Scope) -> Result<StatusReport, AgentSyncError> {
    status_root(std::env::current_dir()?, scope)
}

pub fn status_root(root: impl AsRef<Path>, scope: Scope) -> Result<StatusReport, AgentSyncError> {
    let root = root.as_ref();
    let scan = scan_root(root, scope)?;
    let state = load_state(root)?;
    let mut items = Vec::new();
    for resource in &scan.normalized {
        let checksum = normalized_checksum(resource)?;
        match state
            .resources
            .iter()
            .find(|entry| entry.resource_id == resource.id)
        {
            Some(entry) if entry.source_checksum != checksum => items.push(StatusItem {
                id: resource.id.clone(),
                kind: resource.kind,
                state: DriftState::ChangedSource,
                message: "source normalized content changed".to_string(),
            }),
            Some(entry) => {
                let missing = entry
                    .targets
                    .iter()
                    .find(|target| !root.join(&target.path).exists());
                if let Some(target) = missing {
                    items.push(StatusItem {
                        id: resource.id.clone(),
                        kind: resource.kind,
                        state: DriftState::MissingTarget,
                        message: format!("missing {}", target.path.display()),
                    });
                    continue;
                }
                let stale = entry.targets.iter().find(|target| {
                    fs::read(root.join(&target.path))
                        .map(|bytes| sha256_bytes(&bytes) != target.native_checksum)
                        .unwrap_or(true)
                });
                if let Some(target) = stale {
                    items.push(StatusItem {
                        id: resource.id.clone(),
                        kind: resource.kind,
                        state: DriftState::StaleTarget,
                        message: format!("target drifted {}", target.path.display()),
                    });
                } else {
                    items.push(StatusItem {
                        id: resource.id.clone(),
                        kind: resource.kind,
                        state: DriftState::Clean,
                        message: "tracked and clean".to_string(),
                    });
                }
            }
            None => items.push(StatusItem {
                id: resource.id.clone(),
                kind: resource.kind,
                state: DriftState::Untracked,
                message: "not present in .agentsync/state.json".to_string(),
            }),
        }
    }
    Ok(StatusReport {
        scope,
        items,
        diagnostics: scan.diagnostics,
    })
}

pub fn plan(
    root: impl AsRef<Path>,
    selector: ResourceSelector,
    from: SourceAlias,
    targets: &[Agent],
) -> Result<PlanReport, AgentSyncError> {
    let root = root.as_ref();
    let scan = scan_root(root, Scope::Project)?;
    let state = load_state(root)?;
    let sources = select_sources(&scan.normalized, selector, from)?;
    let mut actions = Vec::new();
    let mut diagnostics = Vec::new();
    for source in sources {
        for target in targets {
            if source.kind == ResourceKind::Skill && source.support != SupportLevel::Portable {
                actions.push(block_action(
                    source,
                    *target,
                    "skill contains blocked native behavior",
                ));
                continue;
            }
            let rendered = render_for_target(root, source, *target)?;
            for file in rendered {
                let path = root.join(&file.path);
                let action = if path.exists() {
                    let existing = fs::read_to_string(&path)?;
                    if existing == file.contents {
                        PlanActionKind::Skip
                    } else if target_drifted(root, &state, &file.path)? {
                        PlanActionKind::Block
                    } else {
                        PlanActionKind::Update
                    }
                } else {
                    PlanActionKind::Create
                };
                let diff = matches!(
                    action,
                    PlanActionKind::Create | PlanActionKind::Update | PlanActionKind::Block
                )
                .then(|| unified_diff(&file.path, fs::read_to_string(&path).ok(), &file.contents));
                actions.push(PlanAction {
                    action,
                    path: file.path.clone(),
                    resource_id: source.id.clone(),
                    reason: if action == PlanActionKind::Block {
                        "target has drifted since last sync".to_string()
                    } else {
                        format!("render {} from {}", target.as_str(), source.id)
                    },
                    rendered: Some(file),
                    diff,
                });
            }
        }
    }
    diagnostics.extend(scan.diagnostics);
    Ok(PlanReport {
        actions,
        diagnostics,
    })
}

fn target_drifted(root: &Path, state: &StateFile, path: &Path) -> Result<bool, AgentSyncError> {
    let Some(target) = state
        .resources
        .iter()
        .flat_map(|resource| &resource.targets)
        .find(|target| target.path == path)
    else {
        return Ok(false);
    };
    let current = fs::read(root.join(path))?;
    Ok(sha256_bytes(&current) != target.native_checksum)
}

pub fn write_plan(root: impl AsRef<Path>, report: &PlanReport) -> Result<(), AgentSyncError> {
    let root = root.as_ref();
    if report.has_blocked_actions() {
        return Err(AgentSyncError::InvalidArgument(
            "planned write contains blocked actions".to_string(),
        ));
    }
    for action in &report.actions {
        if !matches!(
            action.action,
            PlanActionKind::Create | PlanActionKind::Update
        ) {
            continue;
        }
        let rendered = action.rendered.as_ref().ok_or_else(|| {
            AgentSyncError::Adapter("write action was missing rendered content".to_string())
        })?;
        let abs = root.join(&rendered.path);
        if abs.exists() {
            let backup = abs.with_extension(format!(
                "{}bak",
                abs.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| format!("{ext}."))
                    .unwrap_or_default()
            ));
            fs::copy(&abs, backup)?;
        }
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(abs, &rendered.contents)?;
    }
    save_state_from_plan(root, report)
}

fn discover_project(root: &Path) -> Vec<NativeResource> {
    let mut resources = Vec::new();
    push_if_exists(
        &mut resources,
        root,
        Agent::Codex,
        ResourceKind::RuleSet,
        "AGENTS.md",
    );
    push_if_exists(
        &mut resources,
        root,
        Agent::Claude,
        ResourceKind::RuleSet,
        "CLAUDE.md",
    );
    push_if_exists(
        &mut resources,
        root,
        Agent::Claude,
        ResourceKind::RuleSet,
        ".claude/CLAUDE.md",
    );
    push_if_exists(
        &mut resources,
        root,
        Agent::Cursor,
        ResourceKind::RuleSet,
        "AGENTS.md",
    );
    push_if_exists(
        &mut resources,
        root,
        Agent::OpenCode,
        ResourceKind::RuleSet,
        "AGENTS.md",
    );
    push_if_exists(
        &mut resources,
        root,
        Agent::OpenCode,
        ResourceKind::RuleSet,
        "opencode.json",
    );
    discover_glob(
        &mut resources,
        root,
        Agent::Codex,
        ResourceKind::Skill,
        [".codex/skills", ".agents/skills"],
    );
    discover_glob(
        &mut resources,
        root,
        Agent::Claude,
        ResourceKind::Skill,
        [".claude/skills"],
    );
    discover_glob(
        &mut resources,
        root,
        Agent::Cursor,
        ResourceKind::Skill,
        [".cursor/skills"],
    );
    discover_glob(
        &mut resources,
        root,
        Agent::OpenCode,
        ResourceKind::Skill,
        [".opencode/skills"],
    );
    discover_md_dir(
        &mut resources,
        root,
        Agent::Cursor,
        ResourceKind::RuleSet,
        ".cursor/rules",
    );
    discover_md_dir(
        &mut resources,
        root,
        Agent::Claude,
        ResourceKind::Subagent,
        ".claude/agents",
    );
    discover_md_dir(
        &mut resources,
        root,
        Agent::OpenCode,
        ResourceKind::Subagent,
        ".opencode/agents",
    );
    discover_md_dir(
        &mut resources,
        root,
        Agent::OpenCode,
        ResourceKind::Command,
        ".opencode/commands",
    );
    resources.sort_by_key(resource_sort_key);
    resources.dedup_by(|a, b| a.id == b.id && a.agent == b.agent);
    resources
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

fn discover_glob<const N: usize>(
    resources: &mut Vec<NativeResource>,
    root: &Path,
    agent: Agent,
    kind: ResourceKind,
    dirs: [&str; N],
) {
    for dir in dirs {
        let abs = root.join(dir);
        if let Ok(entries) = fs::read_dir(abs) {
            for entry in entries.flatten() {
                let skill = entry.path().join("SKILL.md");
                if skill.is_file() {
                    if let Ok(rel) = skill.strip_prefix(root) {
                        resources.push(native(agent, kind, &rel.to_string_lossy()));
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

fn normalize(root: &Path, native: &NativeResource) -> Result<NormalizedResource, AgentSyncError> {
    match native.kind {
        ResourceKind::RuleSet => normalize_rule(root, native),
        ResourceKind::Skill => normalize_skill(root, native),
        ResourceKind::Subagent | ResourceKind::Hook | ResourceKind::Command => {
            Ok(NormalizedResource {
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
            })
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
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            path.parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "skill".to_string());
    let description = frontmatter
        .get("description")
        .and_then(|value| value.as_str())
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

fn split_frontmatter(raw: &str) -> (BTreeMap<String, serde_json::Value>, String) {
    if let Some(rest) = raw.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            let yaml_like = &rest[..end];
            let body = rest[end + 5..].to_string();
            let mut map = BTreeMap::new();
            for line in yaml_like.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    map.insert(
                        key.trim().to_string(),
                        serde_json::Value::String(value.trim().trim_matches('"').to_string()),
                    );
                }
            }
            return (map, body);
        }
    }
    (BTreeMap::new(), raw.to_string())
}

fn select_sources(
    resources: &[NormalizedResource],
    selector: ResourceSelector,
    from: SourceAlias,
) -> Result<Vec<&NormalizedResource>, AgentSyncError> {
    let kind = match selector {
        ResourceSelector::Rules => ResourceKind::RuleSet,
        ResourceSelector::Skills => ResourceKind::Skill,
    };
    let mut selected = resources
        .iter()
        .filter(|resource| resource.kind == kind)
        .filter(|resource| match from {
            SourceAlias::AgentsMd => resource
                .native_paths
                .iter()
                .any(|path| path == Path::new("AGENTS.md")),
            SourceAlias::Codex => resource.source_agent == Agent::Codex,
            SourceAlias::Claude => resource.source_agent == Agent::Claude,
            SourceAlias::Cursor => resource.source_agent == Agent::Cursor,
            SourceAlias::OpenCode => resource.source_agent == Agent::OpenCode,
        })
        .collect::<Vec<_>>();
    if selector == ResourceSelector::Rules {
        selected.truncate(1);
    }
    if selected.is_empty() {
        Err(AgentSyncError::InvalidArgument(
            "requested source resource was not found".to_string(),
        ))
    } else {
        Ok(selected)
    }
}

fn render_for_target(
    root: &Path,
    source: &NormalizedResource,
    target: Agent,
) -> Result<Vec<RenderedFile>, AgentSyncError> {
    match source.kind {
        ResourceKind::RuleSet => {
            let body = &source
                .rule_set
                .as_ref()
                .ok_or_else(|| AgentSyncError::Adapter("missing rule body".to_string()))?
                .body;
            let path = match target {
                Agent::Codex | Agent::OpenCode => PathBuf::from("AGENTS.md"),
                Agent::Claude => PathBuf::from("CLAUDE.md"),
                Agent::Cursor => PathBuf::from(".cursor/rules/agentsync.md"),
            };
            Ok(vec![RenderedFile {
                path,
                contents: body.clone(),
            }])
        }
        ResourceKind::Skill => {
            let skill = source
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
            let _ = root;
            Ok(vec![RenderedFile { path, contents }])
        }
        _ => Err(AgentSyncError::InvalidArgument(
            "only rules and skills can be rendered in the MVP".to_string(),
        )),
    }
}

fn block_action(source: &NormalizedResource, target: Agent, reason: &str) -> PlanAction {
    PlanAction {
        action: PlanActionKind::Block,
        path: PathBuf::from(format!("{}:{}", target.as_str(), source.id)),
        resource_id: source.id.clone(),
        reason: reason.to_string(),
        rendered: None,
        diff: None,
    }
}

fn unified_diff(path: &Path, old: Option<String>, new: &str) -> String {
    let old = old.unwrap_or_default();
    let mut out = format!("--- a/{}\n+++ b/{}\n", path.display(), path.display());
    out.push_str("@@\n");
    for line in old.lines() {
        out.push_str(&format!("-{line}\n"));
    }
    for line in new.lines() {
        out.push_str(&format!("+{line}\n"));
    }
    out
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StateFile {
    version: u32,
    resources: Vec<StateResource>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StateResource {
    resource_id: String,
    kind: ResourceKind,
    source_paths: Vec<PathBuf>,
    source_checksum: String,
    targets: Vec<StateTarget>,
    last_synced_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StateTarget {
    agent: Agent,
    path: PathBuf,
    native_checksum: String,
}

fn load_state(root: &Path) -> Result<StateFile, AgentSyncError> {
    let path = root.join(".agentsync/state.json");
    if !path.exists() {
        return Ok(StateFile {
            version: 1,
            resources: Vec::new(),
        });
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn save_state_from_plan(root: &Path, report: &PlanReport) -> Result<(), AgentSyncError> {
    let scan = scan_root(root, Scope::Project)?;
    let mut state = load_state(root)?;
    let now = Utc::now().to_rfc3339();
    let mut by_resource: BTreeMap<String, Vec<StateTarget>> = BTreeMap::new();
    for action in &report.actions {
        let Some(rendered) = &action.rendered else {
            continue;
        };
        if !matches!(
            action.action,
            PlanActionKind::Create | PlanActionKind::Update
        ) {
            continue;
        }
        let native_checksum = sha256_bytes(&fs::read(root.join(&rendered.path))?);
        by_resource
            .entry(action.resource_id.clone())
            .or_default()
            .push(StateTarget {
                agent: infer_agent_from_path(&rendered.path),
                path: rendered.path.clone(),
                native_checksum,
            });
    }
    for (resource_id, targets) in by_resource {
        let Some(source) = scan
            .normalized
            .iter()
            .find(|resource| resource.id == resource_id)
        else {
            continue;
        };
        let source_checksum = normalized_checksum(source)?;
        state
            .resources
            .retain(|entry| entry.resource_id != source.id);
        state.resources.push(StateResource {
            resource_id: source.id.clone(),
            kind: source.kind,
            source_paths: source.native_paths.clone(),
            source_checksum,
            targets,
            last_synced_at: now.clone(),
        });
    }
    state
        .resources
        .sort_by(|a, b| a.resource_id.cmp(&b.resource_id));
    let path = root.join(".agentsync/state.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(&state)?)?;
    Ok(())
}

fn infer_agent_from_path(path: &Path) -> Agent {
    let text = path.to_string_lossy();
    if text.starts_with(".claude/") || text == "CLAUDE.md" {
        Agent::Claude
    } else if text.starts_with(".cursor/") {
        Agent::Cursor
    } else if text.starts_with(".opencode/") {
        Agent::OpenCode
    } else {
        Agent::Codex
    }
}

fn normalized_checksum(resource: &NormalizedResource) -> Result<String, AgentSyncError> {
    Ok(sha256_bytes(&serde_json::to_vec(resource)?))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn resource_sort_key(resource: &NativeResource) -> (ResourceKind, PathBuf, Agent) {
    (resource.kind, resource.path.clone(), resource.agent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn scan_discovers_rules_and_skills_deterministically() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();
        fs::create_dir_all(dir.path().join(".claude/skills/review")).unwrap();
        fs::write(
            dir.path().join(".claude/skills/review/SKILL.md"),
            "---\nname: review\ndescription: Review code\n---\nBody\n",
        )
        .unwrap();

        let report = scan_root(dir.path(), Scope::Project).unwrap();

        assert!(report
            .resources
            .iter()
            .any(|resource| resource.path == Path::new("AGENTS.md")));
        assert!(report
            .normalized
            .iter()
            .any(|resource| resource.id == "skills:review"));
        let mut sorted = report.resources.clone();
        sorted.sort_by_key(resource_sort_key);
        assert_eq!(report.resources, sorted);
    }

    #[test]
    fn user_scope_reports_not_implemented() {
        let dir = tempdir().unwrap();
        let report = scan_root(dir.path(), Scope::User).unwrap();

        assert!(report.resources.is_empty());
        assert_eq!(report.diagnostics.len(), 1);
        assert!(report.diagnostics[0]
            .message
            .contains("user scope scanning is not implemented yet"));
    }

    #[test]
    fn sync_write_creates_target_and_state() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Claude],
        )
        .unwrap();
        assert_eq!(report.actions[0].action, PlanActionKind::Create);
        write_plan(dir.path(), &report).unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap(),
            "repo rules\n"
        );
        assert!(dir.path().join(".agentsync/state.json").exists());
        let status = status_root(dir.path(), Scope::Project).unwrap();
        assert!(status
            .items
            .iter()
            .any(|item| item.state == DriftState::Clean));
    }

    #[test]
    fn drifted_target_is_blocked() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();
        let report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Claude],
        )
        .unwrap();
        write_plan(dir.path(), &report).unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "local edit\n").unwrap();
        fs::write(dir.path().join("AGENTS.md"), "new repo rules\n").unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Claude],
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert!(write_plan(dir.path(), &report).is_err());
    }
}
