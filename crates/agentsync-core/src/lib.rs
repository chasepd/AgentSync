pub mod adapters;
pub mod diagnostics;
pub mod model;
pub mod report;
pub mod state;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use adapters::built_in_adapters;
use chrono::Utc;
use model::{NativeResource, NormalizedResource, SupportLevel};
use report::{
    DriftState, PlanAction, PlanActionKind, PlanReport, ScanReport, StatusItem, StatusReport,
};
use sha2::{Digest, Sha256};
use state::{load_state, save_state, StateFile, StateResource, StateTarget};

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
        for adapter in built_in_adapters() {
            report.capabilities.push(adapter.capabilities());
            report
                .diagnostics
                .extend(adapter.validate(root, Scope::Project)?);
            for native in adapter.discover(root, Scope::Project)? {
                match adapter.read(root, &native) {
                    Ok(resource) => report.normalized.push(resource),
                    Err(error) => report.diagnostics.push(Diagnostic {
                        severity: DiagnosticSeverity::Warning,
                        resource_id: Some(native.id.clone()),
                        resource_kind: Some(native.kind),
                        agent: Some(native.agent),
                        message: error.to_string(),
                    }),
                }
                report.resources.push(native);
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
        if resource.support == SupportLevel::Blocked {
            items.push(StatusItem {
                id: resource.id.clone(),
                kind: resource.kind,
                state: DriftState::Blocked,
                message: "resource is blocked for MVP sync".to_string(),
            });
            continue;
        }
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
            let (rendered, render_diagnostics) = render_for_target(source, *target)?;
            diagnostics.extend(render_diagnostics);
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
    if report.has_blocking_issues() {
        return Err(AgentSyncError::InvalidArgument(
            "planned write contains blocking issues".to_string(),
        ));
    }
    let mut wrote_anything = false;
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
            fs::copy(&abs, backup_path(&abs))?;
        }
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(abs, &rendered.contents)?;
        wrote_anything = true;
    }
    if !wrote_anything {
        return Ok(());
    }
    save_state_from_plan(root, report)
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
    source: &NormalizedResource,
    target: Agent,
) -> Result<(Vec<model::RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
    let adapters = built_in_adapters();
    let adapter = adapters
        .iter()
        .find(|adapter| adapter.agent() == target)
        .ok_or_else(|| {
            AgentSyncError::Adapter(format!("missing adapter for {}", target.as_str()))
        })?;
    adapter.render(source)
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

fn backup_path(path: &Path) -> PathBuf {
    let candidate = path.with_extension(format!(
        "{}bak",
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| format!("{ext}."))
            .unwrap_or_default()
    ));
    if !candidate.exists() {
        return candidate;
    }

    for index in 2.. {
        let candidate = path.with_extension(format!(
            "{}bak.{index}",
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| format!("{ext}."))
                .unwrap_or_default()
        ));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("unbounded backup suffix search should always return")
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
    if by_resource.is_empty() {
        return Ok(());
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
        if let Some(entry) = state.resource_mut(&source.id) {
            entry.kind = source.kind;
            entry.source_paths = source.native_paths.clone();
            entry.source_checksum = source_checksum;
            entry.last_synced_at = now.clone();
            for target in targets {
                entry.upsert_target(target);
            }
        } else {
            state.resources.push(StateResource {
                resource_id: source.id.clone(),
                kind: source.kind,
                source_paths: source.native_paths.clone(),
                source_checksum,
                targets,
                last_synced_at: now.clone(),
            });
        }
    }
    state
        .resources
        .sort_by(|a, b| a.resource_id.cmp(&b.resource_id));
    save_state(root, &state)
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
        assert!(report.capabilities.is_empty());
        assert!(report.diagnostics[0]
            .message
            .contains("user scope scanning is not implemented yet"));
    }

    #[test]
    fn untracked_status_is_not_blocking() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

        let status = status_root(dir.path(), Scope::Project).unwrap();

        assert!(status
            .items
            .iter()
            .any(|item| item.state == DriftState::Untracked));
        assert!(!status.has_blocking_issues());
    }

    #[test]
    fn blocked_behavioral_resource_is_blocking_status() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".opencode/commands")).unwrap();
        fs::write(dir.path().join(".opencode/commands/deploy.md"), "deploy\n").unwrap();

        let status = status_root(dir.path(), Scope::Project).unwrap();

        assert!(status.items.iter().any(|item| {
            item.kind == ResourceKind::Command && item.state == DriftState::Blocked
        }));
        assert!(status.has_blocking_issues());
    }

    #[test]
    fn info_diagnostics_are_not_blocking_status() {
        let dir = tempdir().unwrap();

        let status = status_root(dir.path(), Scope::User).unwrap();

        assert!(!status.diagnostics.is_empty());
        assert!(!status.has_blocking_issues());
    }

    #[test]
    fn scan_reports_builtin_capabilities() {
        let dir = tempdir().unwrap();
        let report = scan_root(dir.path(), Scope::Project).unwrap();

        assert_eq!(report.capabilities.len(), 4);
        assert!(report
            .capabilities
            .iter()
            .any(|capabilities| capabilities.agent == Agent::OpenCode));
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

    #[test]
    fn followup_sync_preserves_existing_state_targets() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

        let claude_report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Claude],
        )
        .unwrap();
        write_plan(dir.path(), &claude_report).unwrap();

        let cursor_report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Cursor],
        )
        .unwrap();
        write_plan(dir.path(), &cursor_report).unwrap();

        let state = load_state(dir.path()).unwrap();
        let entry = state
            .resources
            .iter()
            .find(|entry| entry.resource_id == "rules:agents-md")
            .unwrap();

        assert!(entry
            .targets
            .iter()
            .any(|target| target.path == Path::new("CLAUDE.md")));
        assert!(entry
            .targets
            .iter()
            .any(|target| target.path == Path::new(".cursor/rules/agentsync.md")));
    }

    #[test]
    fn repeated_updates_create_unique_backups() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "first\n").unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "existing\n").unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Claude],
        )
        .unwrap();
        write_plan(dir.path(), &report).unwrap();
        fs::write(dir.path().join("AGENTS.md"), "second\n").unwrap();
        let report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Claude],
        )
        .unwrap();
        write_plan(dir.path(), &report).unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join("CLAUDE.md.bak")).unwrap(),
            "existing\n"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("CLAUDE.md.bak.2")).unwrap(),
            "first\n"
        );
    }

    #[test]
    fn skipped_write_does_not_create_empty_state() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "same\n").unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "same\n").unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Claude],
        )
        .unwrap();
        assert_eq!(report.actions[0].action, PlanActionKind::Skip);
        write_plan(dir.path(), &report).unwrap();

        assert!(!dir.path().join(".agentsync/state.json").exists());
    }

    #[test]
    fn write_plan_rejects_blocking_diagnostics() {
        let dir = tempdir().unwrap();
        let report = PlanReport {
            actions: Vec::new(),
            diagnostics: vec![Diagnostic {
                severity: DiagnosticSeverity::Error,
                resource_id: None,
                resource_kind: None,
                agent: None,
                message: "blocked".to_string(),
            }],
        };

        assert!(write_plan(dir.path(), &report).is_err());
        assert!(!dir.path().join(".agentsync/state.json").exists());
    }
}
