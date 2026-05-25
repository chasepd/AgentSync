pub mod adapters;
pub mod config;
pub mod diagnostics;
pub mod model;
pub mod report;
pub mod state;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use config::{config_path, load_config};
use model::{NativeResource, NormalizedResource, SupportLevel};
use report::{
    DoctorReport, DriftState, InitActionKind, InitReport, PlanAction, PlanActionKind, PlanReport,
    ScanReport, StatusItem, StatusReport,
};
use sha2::{Digest, Sha256};
use state::{load_state, save_state, StateFile, StateResource, StateTarget};

pub use adapters::{AdapterRegistry, AgentAdapter};
pub use diagnostics::{AgentSyncError, Diagnostic, DiagnosticSeverity};
pub use model::{
    Agent, ConflictStrategy, DiscoveryRoots, PlanOptions, ResourceFilter, ResourceKind,
    ResourceSelector, Scope, SourceAlias,
};
pub use report::PlanConflictChoice;

pub fn scan(scope: Scope) -> Result<ScanReport, AgentSyncError> {
    scan_roots(default_discovery_roots()?, scope)
}

pub fn scan_root(root: impl AsRef<Path>, scope: Scope) -> Result<ScanReport, AgentSyncError> {
    let root = root.as_ref().to_path_buf();
    scan_roots(
        DiscoveryRoots {
            project: root.clone(),
            user: root,
        },
        scope,
    )
}

pub fn scan_roots(roots: DiscoveryRoots, scope: Scope) -> Result<ScanReport, AgentSyncError> {
    scan_roots_with_adapters(roots, scope, &AdapterRegistry::built_in())
}

pub fn scan_roots_with_adapters(
    roots: DiscoveryRoots,
    scope: Scope,
    adapters: &AdapterRegistry,
) -> Result<ScanReport, AgentSyncError> {
    let mut report = ScanReport::empty(scope);
    if matches!(scope, Scope::Project | Scope::All) {
        scan_scope_root(&roots.project, Scope::Project, &mut report, adapters)?;
    }
    if matches!(scope, Scope::User | Scope::All) {
        scan_scope_root(&roots.user, Scope::User, &mut report, adapters)?;
    }
    report.resources.sort_by_key(resource_sort_key);
    report
        .normalized
        .sort_by(|a, b| (a.scope, &a.id).cmp(&(b.scope, &b.id)));
    report
        .normalized
        .dedup_by(|a, b| a.scope == b.scope && a.id == b.id);
    Ok(report)
}

fn default_discovery_roots() -> Result<DiscoveryRoots, AgentSyncError> {
    let project = std::env::current_dir()?;
    let user = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| project.clone());
    Ok(DiscoveryRoots { project, user })
}

fn scan_scope_root(
    root: &Path,
    scope: Scope,
    report: &mut ScanReport,
    adapters: &AdapterRegistry,
) -> Result<(), AgentSyncError> {
    for adapter in adapters.adapters() {
        if !report
            .capabilities
            .iter()
            .any(|capabilities| capabilities.agent == adapter.agent())
        {
            report.capabilities.push(adapter.capabilities());
        }
        report.diagnostics.extend(adapter.validate(root, scope)?);
        for native in adapter.discover(root, scope)? {
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
    Ok(())
}

pub fn status(scope: Scope) -> Result<StatusReport, AgentSyncError> {
    status_roots(default_discovery_roots()?, scope)
}

pub fn doctor() -> Result<DoctorReport, AgentSyncError> {
    doctor_root(std::env::current_dir()?)
}

pub fn init() -> Result<InitReport, AgentSyncError> {
    init_root(std::env::current_dir()?)
}

pub fn init_root(root: impl AsRef<Path>) -> Result<InitReport, AgentSyncError> {
    let root = root.as_ref();
    let path = PathBuf::from(".agentsync/config.toml");
    let abs = root.join(&path);
    let contents = default_config();
    let (action, message) = if abs.exists() {
        (
            InitActionKind::Skip,
            "config already exists; leaving it untouched".to_string(),
        )
    } else {
        (
            InitActionKind::Create,
            "config is ready to create".to_string(),
        )
    };
    Ok(InitReport {
        path,
        action,
        contents,
        message,
    })
}

pub fn write_init(root: impl AsRef<Path>, report: &InitReport) -> Result<(), AgentSyncError> {
    let root = root.as_ref();
    if report.action != InitActionKind::Create {
        return Ok(());
    }
    let abs = root.join(&report.path);
    if abs.exists() {
        return Ok(());
    }
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(abs, &report.contents)?;
    Ok(())
}

pub fn doctor_root(root: impl AsRef<Path>) -> Result<DoctorReport, AgentSyncError> {
    let root = root.as_ref();
    let scan = scan_root(root, Scope::Project)?;
    let config_path = config_path(root);
    let config_present = config_path.exists();
    let mut config_valid = true;
    let state_path = root.join(".agentsync/state.json");
    let state_present = state_path.exists();
    let mut state_valid = true;
    let mut tracked_resource_count = 0;
    let mut diagnostics = scan.diagnostics.clone();

    if config_present {
        if let Err(error) = load_config(root) {
            config_valid = false;
            diagnostics.push(Diagnostic {
                severity: DiagnosticSeverity::Error,
                resource_id: None,
                resource_kind: None,
                agent: None,
                message: format!("failed to load .agentsync/config.toml: {error}"),
            });
        }
    }

    if state_present {
        match load_state(root) {
            Ok(state) => tracked_resource_count = state.resources.len(),
            Err(error) => {
                state_valid = false;
                diagnostics.push(Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    resource_id: None,
                    resource_kind: None,
                    agent: None,
                    message: format!("failed to load .agentsync/state.json: {error}"),
                });
            }
        }
    }

    let blocking_status_count = if state_valid {
        status_root(root, Scope::Project)?
            .items
            .iter()
            .filter(|item| item.is_blocking())
            .count()
    } else {
        0
    };

    Ok(DoctorReport {
        root: root.to_path_buf(),
        config_path,
        config_present,
        config_valid,
        state_path,
        state_present,
        state_valid,
        resource_count: scan.resources.len(),
        normalized_count: scan.normalized.len(),
        tracked_resource_count,
        blocking_status_count,
        diagnostics,
    })
}

fn default_config() -> String {
    r#"schema_version = 1

[defaults]
scope = "project"
source = "agents-md"
targets = ["claude", "cursor", "opencode"]

[sync]
rules = true
skills = true
subagents = false
commands = false
hooks = false
"#
    .to_string()
}

pub fn status_root(root: impl AsRef<Path>, scope: Scope) -> Result<StatusReport, AgentSyncError> {
    let root = root.as_ref();
    status_roots(
        DiscoveryRoots {
            project: root.to_path_buf(),
            user: root.to_path_buf(),
        },
        scope,
    )
}

pub fn status_roots(roots: DiscoveryRoots, scope: Scope) -> Result<StatusReport, AgentSyncError> {
    let scan = scan_roots(roots.clone(), scope)?;
    let root = &roots.project;
    let state = load_state(root)?;
    let mut items = Vec::new();
    for resource in &scan.normalized {
        if resource.support == SupportLevel::Blocked {
            items.push(StatusItem {
                id: resource.id.clone(),
                kind: resource.kind,
                state: DriftState::Blocked,
                message: "resource is blocked for MVP sync".to_string(),
                suggested_command: None,
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
                suggested_command: suggested_diff_command(resource, entry),
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
                        suggested_command: suggested_diff_command(resource, entry),
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
                        suggested_command: suggested_diff_command(resource, entry),
                    });
                } else {
                    items.push(StatusItem {
                        id: resource.id.clone(),
                        kind: resource.kind,
                        state: DriftState::Clean,
                        message: "tracked and clean".to_string(),
                        suggested_command: None,
                    });
                }
            }
            None => items.push(StatusItem {
                id: resource.id.clone(),
                kind: resource.kind,
                state: DriftState::Untracked,
                message: "not present in .agentsync/state.json".to_string(),
                suggested_command: None,
            }),
        }
    }
    Ok(StatusReport {
        scope,
        items,
        diagnostics: scan.diagnostics,
    })
}

fn suggested_diff_command(resource: &NormalizedResource, entry: &StateResource) -> Option<String> {
    if !matches!(
        resource.kind,
        ResourceKind::RuleSet
            | ResourceKind::Skill
            | ResourceKind::Subagent
            | ResourceKind::Command
    ) {
        return None;
    }
    if entry.targets.is_empty() {
        return None;
    }
    let from = source_alias_for_resource(resource);
    let to = entry
        .targets
        .iter()
        .map(|target| target.agent.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let name = resource_short_name(resource)
        .map(|name| format!(" {name}"))
        .unwrap_or_default();
    Some(format!(
        "agentsync diff {}{} --from {from} --to {to}",
        resource.kind.as_str(),
        name
    ))
}

fn resource_short_name(resource: &NormalizedResource) -> Option<&str> {
    match resource.kind {
        ResourceKind::Skill => resource.skill.as_ref().map(|skill| skill.name.as_str()),
        ResourceKind::Subagent => resource
            .subagent
            .as_ref()
            .map(|subagent| subagent.name.as_str()),
        ResourceKind::Command => resource
            .command
            .as_ref()
            .map(|command| command.name.as_str()),
        _ => None,
    }
}

fn source_alias_for_resource(resource: &NormalizedResource) -> &'static str {
    if resource
        .native_paths
        .iter()
        .any(|path| path == Path::new("AGENTS.md"))
    {
        return "agents-md";
    }
    resource.source_agent.as_str()
}

pub fn plan(
    root: impl AsRef<Path>,
    selector: ResourceSelector,
    from: SourceAlias,
    targets: &[Agent],
) -> Result<PlanReport, AgentSyncError> {
    plan_filtered(root, ResourceFilter::all(selector), from, targets)
}

pub fn plan_filtered(
    root: impl AsRef<Path>,
    filter: ResourceFilter,
    from: SourceAlias,
    targets: &[Agent],
) -> Result<PlanReport, AgentSyncError> {
    plan_filtered_with_options(root, filter, from, targets, PlanOptions::default())
}

pub fn plan_filtered_with_options(
    root: impl AsRef<Path>,
    filter: ResourceFilter,
    from: SourceAlias,
    targets: &[Agent],
    options: PlanOptions,
) -> Result<PlanReport, AgentSyncError> {
    plan_filtered_with_options_and_adapters(
        root,
        filter,
        from,
        targets,
        options,
        &AdapterRegistry::built_in(),
    )
}

pub fn plan_filtered_with_options_and_adapters(
    root: impl AsRef<Path>,
    filter: ResourceFilter,
    from: SourceAlias,
    targets: &[Agent],
    options: PlanOptions,
    adapters: &AdapterRegistry,
) -> Result<PlanReport, AgentSyncError> {
    let root = root.as_ref();
    let scan = scan_roots_with_adapters(
        DiscoveryRoots {
            project: root.to_path_buf(),
            user: root.to_path_buf(),
        },
        Scope::Project,
        adapters,
    )?;
    let state = load_state(root)?;
    let sources = select_sources(&scan.normalized, &filter, from)?;
    let mut actions = Vec::new();
    let mut diagnostics = Vec::new();
    for source in sources {
        for target in targets {
            if !matches!(
                source.kind,
                ResourceKind::RuleSet
                    | ResourceKind::Skill
                    | ResourceKind::Subagent
                    | ResourceKind::Command
            ) {
                diagnostics.extend(source.diagnostics.clone());
                actions.push(block_action(source, *target, "non-portable"));
                continue;
            }
            if source.kind == ResourceKind::Skill && source.support != SupportLevel::Portable {
                diagnostics.extend(source.diagnostics.clone());
                actions.push(block_action(source, *target, "non-portable"));
                continue;
            }
            if source.kind == ResourceKind::Subagent
                && (source.support == SupportLevel::Blocked || *target == Agent::CursorCli)
            {
                diagnostics.extend(source.diagnostics.clone());
                actions.push(block_action(source, *target, "non-portable"));
                continue;
            }
            if source.kind == ResourceKind::Command
                && (source.support == SupportLevel::Blocked || *target != Agent::OpenCode)
            {
                diagnostics.extend(source.diagnostics.clone());
                actions.push(block_action(source, *target, "non-portable"));
                continue;
            }
            let (rendered, render_diagnostics) = render_for_target(source, *target, adapters)?;
            diagnostics.extend(render_diagnostics);
            for file in rendered {
                let path = root.join(&file.path);
                let (action, reason) = if path.exists() {
                    let existing = fs::read_to_string(&path)?;
                    if existing == file.contents {
                        (
                            PlanActionKind::Skip,
                            format!("render {} from {}", target.as_str(), source.id),
                        )
                    } else if options.no_overwrite {
                        (PlanActionKind::Block, "target exists".to_string())
                    } else if target_drifted(root, &state, &file.path)? {
                        resolve_target_drift(root, source, &file.path, &options)?
                    } else {
                        (
                            PlanActionKind::Update,
                            format!("render {} from {}", target.as_str(), source.id),
                        )
                    }
                } else {
                    (
                        PlanActionKind::Create,
                        format!("render {} from {}", target.as_str(), source.id),
                    )
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
                    reason,
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

fn resolve_target_drift(
    root: &Path,
    source: &NormalizedResource,
    target_path: &Path,
    options: &PlanOptions,
) -> Result<(PlanActionKind, String), AgentSyncError> {
    match options.strategy {
        ConflictStrategy::Conservative => Ok((PlanActionKind::Block, "target drifted".to_string())),
        ConflictStrategy::Source => Ok((
            PlanActionKind::Update,
            format!("render source from {}", source.id),
        )),
        ConflictStrategy::Newest => {
            let source_mtime = source_modified_time(root, source)?;
            let target_mtime = fs::metadata(root.join(target_path))?.modified()?;
            if source_mtime > target_mtime {
                Ok((
                    PlanActionKind::Update,
                    format!("source newer than target for {}", source.id),
                ))
            } else {
                Ok((PlanActionKind::Block, "target drifted".to_string()))
            }
        }
    }
}

fn source_modified_time(
    root: &Path,
    source: &NormalizedResource,
) -> Result<std::time::SystemTime, AgentSyncError> {
    let mut newest = std::time::SystemTime::UNIX_EPOCH;
    for path in &source.native_paths {
        let modified = fs::metadata(root.join(path))?.modified()?;
        newest = newest.max(modified);
    }
    if let Some(skill) = &source.skill {
        for path in &skill.asset_paths {
            let modified = fs::metadata(root.join(path))?.modified()?;
            newest = newest.max(modified);
        }
    }
    Ok(newest)
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

pub fn resolve_plan_conflict(
    report: &mut PlanReport,
    action_index: usize,
    choice: PlanConflictChoice,
) -> Result<(), AgentSyncError> {
    let action = report.actions.get_mut(action_index).ok_or_else(|| {
        AgentSyncError::InvalidArgument(format!("plan action index {action_index} was not found"))
    })?;
    if !is_interactive_conflict(action) {
        return Err(AgentSyncError::InvalidArgument(format!(
            "plan action {} is not an interactive conflict",
            action.path.display()
        )));
    }
    match choice {
        PlanConflictChoice::Source => {
            action.action = PlanActionKind::Update;
            action.reason = "interactive: keep source".to_string();
        }
        PlanConflictChoice::Target => {
            action.action = PlanActionKind::Skip;
            action.reason = "interactive: keep target".to_string();
            action.rendered = None;
            action.diff = None;
        }
        PlanConflictChoice::Skip => {
            action.action = PlanActionKind::Skip;
            action.reason = "interactive: skipped".to_string();
            action.rendered = None;
            action.diff = None;
        }
    }
    Ok(())
}

pub fn is_interactive_conflict(action: &PlanAction) -> bool {
    action.action == PlanActionKind::Block
        && matches!(action.reason.as_str(), "target drifted" | "target exists")
        && action.rendered.is_some()
}

fn select_sources<'a>(
    resources: &'a [NormalizedResource],
    filter: &ResourceFilter,
    from: SourceAlias,
) -> Result<Vec<&'a NormalizedResource>, AgentSyncError> {
    let kind = match filter.selector {
        ResourceSelector::Rules => ResourceKind::RuleSet,
        ResourceSelector::Skills => ResourceKind::Skill,
        ResourceSelector::Subagents => ResourceKind::Subagent,
        ResourceSelector::Commands => ResourceKind::Command,
        ResourceSelector::Hooks => ResourceKind::Hook,
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
            SourceAlias::CursorCli => resource.source_agent == Agent::CursorCli,
            SourceAlias::OpenCode => resource.source_agent == Agent::OpenCode,
        })
        .filter(|resource| {
            filter
                .name
                .as_deref()
                .is_none_or(|name| resource_matches_name(resource, name))
        })
        .collect::<Vec<_>>();
    if filter.selector == ResourceSelector::Rules && filter.name.is_none() {
        selected.truncate(1);
    }
    if selected.is_empty() {
        let suffix = filter
            .name
            .as_ref()
            .map(|name| format!(" named {name:?}"))
            .unwrap_or_default();
        Err(AgentSyncError::InvalidArgument(format!(
            "requested source resource{} was not found",
            suffix
        )))
    } else {
        Ok(selected)
    }
}

fn resource_matches_name(resource: &NormalizedResource, name: &str) -> bool {
    resource.id == name
        || resource.id.rsplit(':').next() == Some(name)
        || resource
            .skill
            .as_ref()
            .is_some_and(|skill| skill.name == name)
        || resource
            .subagent
            .as_ref()
            .is_some_and(|subagent| subagent.name == name)
        || resource
            .command
            .as_ref()
            .is_some_and(|command| command.name == name)
        || resource.native_paths.iter().any(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem == name)
        })
}

fn render_for_target(
    source: &NormalizedResource,
    target: Agent,
    adapters: &AdapterRegistry,
) -> Result<(Vec<model::RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
    let adapter = adapters
        .adapters()
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
    let old_lines = old.lines().collect::<Vec<_>>();
    let new_lines = new.lines().collect::<Vec<_>>();
    let rows = diff_rows(&old_lines, &new_lines);
    let hunks = diff_hunks(&rows, 3);

    let mut out = format!("--- a/{}\n+++ b/{}\n", path.display(), path.display());
    for hunk in hunks {
        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
        ));
        for row in &rows[hunk.start..hunk.end] {
            out.push(row.kind.prefix());
            out.push_str(row.text);
            out.push('\n');
        }
    }
    out
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffKind {
    Equal,
    Delete,
    Insert,
}

impl DiffKind {
    fn prefix(self) -> char {
        match self {
            Self::Equal => ' ',
            Self::Delete => '-',
            Self::Insert => '+',
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DiffRow<'a> {
    kind: DiffKind,
    text: &'a str,
    old_line: Option<usize>,
    new_line: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DiffHunk {
    start: usize,
    end: usize,
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
}

fn diff_rows<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<DiffRow<'a>> {
    let mut lcs = vec![vec![0; new.len() + 1]; old.len() + 1];
    for old_index in (0..old.len()).rev() {
        for new_index in (0..new.len()).rev() {
            lcs[old_index][new_index] = if old[old_index] == new[new_index] {
                lcs[old_index + 1][new_index + 1] + 1
            } else {
                lcs[old_index + 1][new_index].max(lcs[old_index][new_index + 1])
            };
        }
    }

    let mut rows = Vec::new();
    let mut old_index = 0;
    let mut new_index = 0;
    let mut old_line = 1;
    let mut new_line = 1;
    while old_index < old.len() && new_index < new.len() {
        if old[old_index] == new[new_index] {
            rows.push(DiffRow {
                kind: DiffKind::Equal,
                text: old[old_index],
                old_line: Some(old_line),
                new_line: Some(new_line),
            });
            old_index += 1;
            new_index += 1;
            old_line += 1;
            new_line += 1;
        } else if lcs[old_index + 1][new_index] >= lcs[old_index][new_index + 1] {
            rows.push(DiffRow {
                kind: DiffKind::Delete,
                text: old[old_index],
                old_line: Some(old_line),
                new_line: None,
            });
            old_index += 1;
            old_line += 1;
        } else {
            rows.push(DiffRow {
                kind: DiffKind::Insert,
                text: new[new_index],
                old_line: None,
                new_line: Some(new_line),
            });
            new_index += 1;
            new_line += 1;
        }
    }
    while old_index < old.len() {
        rows.push(DiffRow {
            kind: DiffKind::Delete,
            text: old[old_index],
            old_line: Some(old_line),
            new_line: None,
        });
        old_index += 1;
        old_line += 1;
    }
    while new_index < new.len() {
        rows.push(DiffRow {
            kind: DiffKind::Insert,
            text: new[new_index],
            old_line: None,
            new_line: Some(new_line),
        });
        new_index += 1;
        new_line += 1;
    }
    rows
}

fn diff_hunks(rows: &[DiffRow<'_>], context: usize) -> Vec<DiffHunk> {
    let changed = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.kind != DiffKind::Equal).then_some(index))
        .collect::<Vec<_>>();
    if changed.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::<(usize, usize)>::new();
    for index in changed {
        let start = index.saturating_sub(context);
        let end = (index + context + 1).min(rows.len());
        if let Some((_, previous_end)) = ranges.last_mut() {
            if start <= *previous_end {
                *previous_end = (*previous_end).max(end);
                continue;
            }
        }
        ranges.push((start, end));
    }

    ranges
        .into_iter()
        .map(|(start, end)| {
            let rows = &rows[start..end];
            let old_count = rows.iter().filter(|row| row.old_line.is_some()).count();
            let new_count = rows.iter().filter(|row| row.new_line.is_some()).count();
            DiffHunk {
                start,
                end,
                old_start: hunk_start(rows.iter().filter_map(|row| row.old_line)),
                old_count,
                new_start: hunk_start(rows.iter().filter_map(|row| row.new_line)),
                new_count,
            }
        })
        .collect()
}

fn hunk_start(mut lines: impl Iterator<Item = usize>) -> usize {
    lines.next().unwrap_or(0)
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
            entry.source_agent = Some(source.source_agent);
            entry.source_paths = source.native_paths.clone();
            entry.source_checksum = source_checksum;
            entry.diagnostics = source.diagnostics.clone();
            entry.native_extensions = source.native_extensions.clone();
            entry.last_synced_at = now.clone();
            for target in targets {
                entry.upsert_target(target);
            }
        } else {
            state.resources.push(StateResource {
                resource_id: source.id.clone(),
                kind: source.kind,
                source_agent: Some(source.source_agent),
                source_paths: source.native_paths.clone(),
                source_checksum,
                targets,
                diagnostics: source.diagnostics.clone(),
                native_extensions: source.native_extensions.clone(),
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
        Agent::CursorCli
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

fn resource_sort_key(resource: &NativeResource) -> (Scope, ResourceKind, PathBuf, Agent) {
    (
        resource.scope,
        resource.kind,
        resource.path.clone(),
        resource.agent,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AdapterCapabilities, RenderedFile, RuleSet};
    use std::time::SystemTime;
    use tempfile::tempdir;

    struct ExternalSourceAdapter;

    impl AgentAdapter for ExternalSourceAdapter {
        fn agent(&self) -> Agent {
            Agent::Codex
        }

        fn capabilities(&self) -> AdapterCapabilities {
            let mut resources = BTreeMap::new();
            resources.insert(ResourceKind::RuleSet, SupportLevel::Portable);
            AdapterCapabilities {
                agent: Agent::Codex,
                resources,
                fields: BTreeMap::new(),
            }
        }

        fn discover(
            &self,
            root: &Path,
            scope: Scope,
        ) -> Result<Vec<NativeResource>, AgentSyncError> {
            if scope == Scope::Project && root.join("EXTERNAL.md").is_file() {
                Ok(vec![NativeResource {
                    id: "rules:external".to_string(),
                    agent: Agent::Codex,
                    kind: ResourceKind::RuleSet,
                    scope,
                    path: PathBuf::from("EXTERNAL.md"),
                }])
            } else {
                Ok(Vec::new())
            }
        }

        fn read(
            &self,
            root: &Path,
            native: &NativeResource,
        ) -> Result<NormalizedResource, AgentSyncError> {
            Ok(NormalizedResource {
                id: native.id.clone(),
                kind: native.kind,
                scope: native.scope,
                source_agent: native.agent,
                native_paths: vec![native.path.clone()],
                rule_set: Some(RuleSet {
                    body: fs::read_to_string(root.join(&native.path))?,
                }),
                skill: None,
                subagent: None,
                command: None,
                native_extensions: BTreeMap::new(),
                diagnostics: Vec::new(),
                support: SupportLevel::Portable,
            })
        }

        fn render(
            &self,
            _resource: &NormalizedResource,
        ) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
            Err(AgentSyncError::Adapter(
                "external source adapter does not render".to_string(),
            ))
        }
    }

    struct ExternalTargetAdapter;

    impl AgentAdapter for ExternalTargetAdapter {
        fn agent(&self) -> Agent {
            Agent::Claude
        }

        fn capabilities(&self) -> AdapterCapabilities {
            let mut resources = BTreeMap::new();
            resources.insert(ResourceKind::RuleSet, SupportLevel::Portable);
            AdapterCapabilities {
                agent: Agent::Claude,
                resources,
                fields: BTreeMap::new(),
            }
        }

        fn discover(
            &self,
            _root: &Path,
            _scope: Scope,
        ) -> Result<Vec<NativeResource>, AgentSyncError> {
            Ok(Vec::new())
        }

        fn read(
            &self,
            _root: &Path,
            _native: &NativeResource,
        ) -> Result<NormalizedResource, AgentSyncError> {
            Err(AgentSyncError::Adapter(
                "external target adapter does not read".to_string(),
            ))
        }

        fn render(
            &self,
            resource: &NormalizedResource,
        ) -> Result<(Vec<RenderedFile>, Vec<Diagnostic>), AgentSyncError> {
            let body = resource
                .rule_set
                .as_ref()
                .ok_or_else(|| AgentSyncError::Adapter("missing external rule body".to_string()))?
                .body
                .clone();
            Ok((
                vec![RenderedFile {
                    path: PathBuf::from("EXTERNAL_TARGET.md"),
                    contents: format!("external target\n{body}"),
                }],
                Vec::new(),
            ))
        }
    }

    fn modified_time(path: &Path) -> SystemTime {
        fs::metadata(path).unwrap().modified().unwrap()
    }

    fn write_until_newer(path: &Path, contents: &str, older: SystemTime) {
        for _ in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            fs::write(path, contents).unwrap();
            if modified_time(path) > older {
                return;
            }
        }
        panic!("{} mtime did not advance", path.display());
    }

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
    fn external_adapter_registry_declares_capabilities_and_discovers_resources() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("EXTERNAL.md"), "external rules\n").unwrap();
        let registry = AdapterRegistry::from_adapters(vec![Box::new(ExternalSourceAdapter)]);

        let report = scan_roots_with_adapters(
            DiscoveryRoots {
                project: dir.path().to_path_buf(),
                user: dir.path().to_path_buf(),
            },
            Scope::Project,
            &registry,
        )
        .unwrap();

        assert_eq!(report.capabilities.len(), 1);
        assert_eq!(report.capabilities[0].agent, Agent::Codex);
        assert_eq!(report.resources[0].id, "rules:external");
        assert_eq!(report.normalized[0].id, "rules:external");
        assert_eq!(
            report.normalized[0].rule_set.as_ref().unwrap().body,
            "external rules\n"
        );
    }

    #[test]
    fn external_adapters_render_plan_actions_without_built_ins() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("EXTERNAL.md"), "external rules\n").unwrap();
        let registry = AdapterRegistry::from_adapters(vec![
            Box::new(ExternalSourceAdapter),
            Box::new(ExternalTargetAdapter),
        ]);

        let report = plan_filtered_with_options_and_adapters(
            dir.path(),
            ResourceFilter::all(ResourceSelector::Rules),
            SourceAlias::Codex,
            &[Agent::Claude],
            PlanOptions::default(),
            &registry,
        )
        .unwrap();

        assert_eq!(report.actions.len(), 1);
        assert_eq!(report.actions[0].action, PlanActionKind::Create);
        assert_eq!(report.actions[0].resource_id, "rules:external");
        let rendered = report.actions[0].rendered.as_ref().unwrap();
        assert_eq!(rendered.path, Path::new("EXTERNAL_TARGET.md"));
        assert_eq!(rendered.contents, "external target\nexternal rules\n");
    }

    #[test]
    fn user_scope_discovers_personal_config_from_injected_root() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".codex/skills/review")).unwrap();
        fs::write(dir.path().join(".codex/AGENTS.md"), "codex user rules\n").unwrap();
        fs::write(
            dir.path().join(".codex/skills/review/SKILL.md"),
            "---\nname: review\n---\nBody\n",
        )
        .unwrap();
        fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
        fs::write(
            dir.path().join(".claude/agents/reviewer.md"),
            "---\nname: reviewer\n---\nReview.\n",
        )
        .unwrap();
        fs::create_dir_all(dir.path().join(".config/opencode/commands")).unwrap();
        fs::write(
            dir.path().join(".config/opencode/opencode.json"),
            r#"{"permission":{"edit":"allow"}}"#,
        )
        .unwrap();
        fs::write(
            dir.path().join(".config/opencode/commands/deploy.md"),
            "deploy\n",
        )
        .unwrap();

        let report = scan_root(dir.path(), Scope::User).unwrap();

        assert!(report
            .resources
            .iter()
            .all(|resource| resource.scope == Scope::User));
        assert!(report
            .resources
            .iter()
            .any(|resource| resource.path == Path::new(".codex/AGENTS.md")));
        assert!(report
            .normalized
            .iter()
            .any(|resource| resource.id == "skills:review" && resource.scope == Scope::User));
        assert!(report.normalized.iter().any(|resource| {
            resource.id == "subagents:reviewer" && resource.scope == Scope::User
        }));
        assert!(report.normalized.iter().any(|resource| {
            resource.id == "permissions:opencode:.config/opencode/opencode.json"
                && resource.scope == Scope::User
        }));
        assert!(report.normalized.iter().any(|resource| {
            resource.id == "commands:opencode:.config/opencode/commands/deploy.md"
                && resource.scope == Scope::User
        }));
    }

    #[test]
    fn all_scope_uses_separate_project_and_user_roots() {
        let project = tempdir().unwrap();
        let user = tempdir().unwrap();
        fs::write(project.path().join("AGENTS.md"), "project rules\n").unwrap();
        fs::create_dir_all(project.path().join(".codex/skills/review")).unwrap();
        fs::write(
            project.path().join(".codex/skills/review/SKILL.md"),
            "---\nname: review\n---\nProject body\n",
        )
        .unwrap();
        fs::create_dir_all(user.path().join(".codex")).unwrap();
        fs::write(user.path().join(".codex/AGENTS.md"), "user rules\n").unwrap();
        fs::create_dir_all(user.path().join(".codex/skills/review")).unwrap();
        fs::write(
            user.path().join(".codex/skills/review/SKILL.md"),
            "---\nname: review\n---\nUser body\n",
        )
        .unwrap();

        let report = scan_roots(
            DiscoveryRoots {
                project: project.path().to_path_buf(),
                user: user.path().to_path_buf(),
            },
            Scope::All,
        )
        .unwrap();

        assert!(report.resources.iter().any(|resource| {
            resource.path == Path::new("AGENTS.md") && resource.scope == Scope::Project
        }));
        assert!(report.resources.iter().any(|resource| {
            resource.path == Path::new(".codex/AGENTS.md") && resource.scope == Scope::User
        }));
        assert_eq!(
            report
                .normalized
                .iter()
                .filter(|resource| resource.id == "skills:review")
                .count(),
            2
        );
        assert_eq!(report.capabilities.len(), 4);
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
        fs::write(
            dir.path().join(".opencode/commands/deploy.md"),
            "deploy with !`npm run build`\n",
        )
        .unwrap();

        let status = status_root(dir.path(), Scope::Project).unwrap();

        assert!(status.items.iter().any(|item| {
            item.kind == ResourceKind::Command && item.state == DriftState::Blocked
        }));
        assert!(status.has_blocking_issues());
    }

    #[test]
    fn portable_subagent_status_is_untracked_and_keeps_normalized_data() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
        fs::write(
            dir.path().join(".claude/agents/reviewer.md"),
            "---\nname: reviewer\ndescription: Review code\n---\nReview carefully.\n",
        )
        .unwrap();

        let scan = scan_root(dir.path(), Scope::Project).unwrap();
        let resource = scan
            .normalized
            .iter()
            .find(|resource| resource.id == "subagents:reviewer")
            .unwrap();
        let status = status_root(dir.path(), Scope::Project).unwrap();

        assert_eq!(resource.support, SupportLevel::Portable);
        assert_eq!(
            resource.subagent.as_ref().unwrap().description.as_deref(),
            Some("Review code")
        );
        let untracked = status
            .items
            .iter()
            .any(|item| item.id == "subagents:reviewer" && item.state == DriftState::Untracked);
        assert!(untracked);
    }

    #[test]
    fn info_diagnostics_are_not_blocking_status() {
        let dir = tempdir().unwrap();

        let status = status_root(dir.path(), Scope::User).unwrap();

        assert!(status.diagnostics.is_empty());
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
        assert_eq!(report.actions[0].reason, "target drifted");
        assert!(write_plan(dir.path(), &report).is_err());
    }

    #[test]
    fn interactive_conflict_source_resolution_updates_with_backup() {
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

        let mut report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Claude],
        )
        .unwrap();
        resolve_plan_conflict(&mut report, 0, PlanConflictChoice::Source).unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Update);
        assert!(!report.has_blocking_issues());
        write_plan(dir.path(), &report).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap(),
            "new repo rules\n"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("CLAUDE.md.bak")).unwrap(),
            "local edit\n"
        );
    }

    #[test]
    fn interactive_conflict_target_resolution_skips_write() {
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

        let mut report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Claude],
        )
        .unwrap();
        resolve_plan_conflict(&mut report, 0, PlanConflictChoice::Target).unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Skip);
        assert_eq!(report.actions[0].reason, "interactive: keep target");
        assert!(!report.has_blocking_issues());
        write_plan(dir.path(), &report).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap(),
            "local edit\n"
        );
    }

    #[test]
    fn interactive_resolution_rejects_non_conflict_block() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude")).unwrap();
        fs::write(
            dir.path().join(".claude/settings.json"),
            r#"{"hooks":{"PreToolUse":[]}}"#,
        )
        .unwrap();

        let mut report = plan(
            dir.path(),
            ResourceSelector::Hooks,
            SourceAlias::Claude,
            &[Agent::Codex],
        )
        .unwrap();

        assert!(resolve_plan_conflict(&mut report, 0, PlanConflictChoice::Source).is_err());
        assert_eq!(report.actions[0].action, PlanActionKind::Block);
    }

    #[test]
    fn changed_source_updates_without_conflict_strategy() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("AGENTS.md");
        fs::write(&source, "repo rules\n").unwrap();
        let report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Claude],
        )
        .unwrap();
        write_plan(dir.path(), &report).unwrap();
        write_until_newer(&source, "new repo rules\n", modified_time(&source));

        let report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Claude],
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Update);
        assert_eq!(
            fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap(),
            "repo rules\n"
        );
    }

    #[test]
    fn strategy_source_updates_drifted_target_and_writes_backup() {
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

        let report = plan_filtered_with_options(
            dir.path(),
            ResourceFilter::all(ResourceSelector::Rules),
            SourceAlias::AgentsMd,
            &[Agent::Claude],
            PlanOptions {
                strategy: ConflictStrategy::Source,
                ..PlanOptions::default()
            },
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Update);
        write_plan(dir.path(), &report).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap(),
            "new repo rules\n"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("CLAUDE.md.bak")).unwrap(),
            "local edit\n"
        );
    }

    #[test]
    fn strategy_newest_updates_when_source_is_newer_than_drifted_target() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("AGENTS.md");
        let target = dir.path().join("CLAUDE.md");
        fs::write(&source, "repo rules\n").unwrap();
        let report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Claude],
        )
        .unwrap();
        write_plan(dir.path(), &report).unwrap();
        write_until_newer(&target, "local edit\n", modified_time(&source));
        write_until_newer(&source, "new repo rules\n", modified_time(&target));

        let report = plan_filtered_with_options(
            dir.path(),
            ResourceFilter::all(ResourceSelector::Rules),
            SourceAlias::AgentsMd,
            &[Agent::Claude],
            PlanOptions {
                strategy: ConflictStrategy::Newest,
                ..PlanOptions::default()
            },
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Update);
        assert_eq!(
            report.actions[0].reason,
            "source newer than target for rules:agents-md"
        );
        write_plan(dir.path(), &report).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("CLAUDE.md.bak")).unwrap(),
            "local edit\n"
        );
    }

    #[test]
    fn strategy_newest_blocks_when_target_is_newer_than_source() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("AGENTS.md");
        let target = dir.path().join("CLAUDE.md");
        fs::write(&source, "repo rules\n").unwrap();
        let report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::AgentsMd,
            &[Agent::Claude],
        )
        .unwrap();
        write_plan(dir.path(), &report).unwrap();
        write_until_newer(&source, "new repo rules\n", modified_time(&target));
        write_until_newer(&target, "local edit\n", modified_time(&source));

        let report = plan_filtered_with_options(
            dir.path(),
            ResourceFilter::all(ResourceSelector::Rules),
            SourceAlias::AgentsMd,
            &[Agent::Claude],
            PlanOptions {
                strategy: ConflictStrategy::Newest,
                ..PlanOptions::default()
            },
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert_eq!(report.actions[0].reason, "target drifted");
        assert!(write_plan(dir.path(), &report).is_err());
    }

    #[test]
    fn strategy_newest_creates_missing_target() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();

        let report = plan_filtered_with_options(
            dir.path(),
            ResourceFilter::all(ResourceSelector::Rules),
            SourceAlias::AgentsMd,
            &[Agent::Claude],
            PlanOptions {
                strategy: ConflictStrategy::Newest,
                ..PlanOptions::default()
            },
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Create);
    }

    #[test]
    fn strategy_newest_considers_skill_asset_mtime() {
        let dir = tempdir().unwrap();
        let asset = dir.path().join(".claude/skills/review/assets/guide.md");
        fs::create_dir_all(asset.parent().unwrap()).unwrap();
        let skill_path = dir.path().join(".claude/skills/review/SKILL.md");
        fs::write(&skill_path, "---\nname: review\n---\nReview body\n").unwrap();
        fs::write(&asset, "asset body\n").unwrap();
        write_until_newer(&asset, "new asset body\n", modified_time(&skill_path));
        let resource = scan_root(dir.path(), Scope::Project)
            .unwrap()
            .normalized
            .into_iter()
            .find(|resource| {
                resource.kind == ResourceKind::Skill && resource.source_agent == Agent::Claude
            })
            .unwrap();

        assert_eq!(
            source_modified_time(dir.path(), &resource).unwrap(),
            modified_time(&asset)
        );
    }

    #[test]
    fn conflict_strategy_does_not_unblock_unsupported_behavior() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude")).unwrap();
        fs::write(
            dir.path().join(".claude/settings.json"),
            r#"{"hooks":{"PreToolUse":[]}}"#,
        )
        .unwrap();

        let report = plan_filtered_with_options(
            dir.path(),
            ResourceFilter::all(ResourceSelector::Hooks),
            SourceAlias::Claude,
            &[Agent::Codex],
            PlanOptions {
                strategy: ConflictStrategy::Source,
                ..PlanOptions::default()
            },
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert_eq!(report.actions[0].reason, "non-portable");
        assert!(write_plan(dir.path(), &report).is_err());
    }

    #[test]
    fn no_overwrite_blocks_existing_untracked_target() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "repo rules\n").unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "existing local rules\n").unwrap();

        let report = plan_filtered_with_options(
            dir.path(),
            ResourceFilter::all(ResourceSelector::Rules),
            SourceAlias::AgentsMd,
            &[Agent::Claude],
            PlanOptions {
                no_overwrite: true,
                ..PlanOptions::default()
            },
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert_eq!(report.actions[0].reason, "target exists");
        assert!(write_plan(dir.path(), &report).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap(),
            "existing local rules\n"
        );
        assert!(!dir.path().join("CLAUDE.md.bak").exists());
        assert!(!dir.path().join(".agentsync/state.json").exists());
    }

    #[test]
    fn no_overwrite_blocks_even_with_source_conflict_strategy() {
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

        let report = plan_filtered_with_options(
            dir.path(),
            ResourceFilter::all(ResourceSelector::Rules),
            SourceAlias::AgentsMd,
            &[Agent::Claude],
            PlanOptions {
                no_overwrite: true,
                strategy: ConflictStrategy::Source,
            },
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert_eq!(report.actions[0].reason, "target exists");
        assert!(write_plan(dir.path(), &report).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap(),
            "local edit\n"
        );
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
            &[Agent::CursorCli],
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
    fn sync_state_records_source_metadata() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "readme rules\n").unwrap();
        fs::write(
            dir.path().join("opencode.json"),
            r#"{"instructions":["README.md","*.md"]}"#,
        )
        .unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Rules,
            SourceAlias::OpenCode,
            &[Agent::Claude],
        )
        .unwrap();
        write_plan(dir.path(), &report).unwrap();

        let state = load_state(dir.path()).unwrap();
        let entry = state
            .resources
            .iter()
            .find(|entry| entry.resource_id == "rules:opencode:opencode.json")
            .unwrap();

        assert_eq!(entry.source_agent, Some(Agent::OpenCode));
        assert!(entry
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("uses a glob")));
        assert!(entry.native_extensions.contains_key("opencode.config"));
    }

    #[test]
    fn skill_sync_writes_text_assets() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude/skills/review/assets")).unwrap();
        fs::write(
            dir.path().join(".claude/skills/review/SKILL.md"),
            "---\nname: review\ndescription: Review code\n---\nBody\n",
        )
        .unwrap();
        fs::write(
            dir.path().join(".claude/skills/review/assets/guide.md"),
            "asset body\n",
        )
        .unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Skills,
            SourceAlias::Claude,
            &[Agent::Codex],
        )
        .unwrap();
        assert!(report
            .actions
            .iter()
            .any(|action| action.path == Path::new(".codex/skills/review/SKILL.md")));
        assert!(report
            .actions
            .iter()
            .any(|action| action.path == Path::new(".codex/skills/review/assets/guide.md")));

        write_plan(dir.path(), &report).unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join(".codex/skills/review/assets/guide.md")).unwrap(),
            "asset body\n"
        );
    }

    #[test]
    fn resource_filter_limits_plan_to_named_skill() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude/skills/review")).unwrap();
        fs::create_dir_all(dir.path().join(".claude/skills/lint")).unwrap();
        fs::write(
            dir.path().join(".claude/skills/review/SKILL.md"),
            "---\nname: review\n---\nReview body\n",
        )
        .unwrap();
        fs::write(
            dir.path().join(".claude/skills/lint/SKILL.md"),
            "---\nname: lint\n---\nLint body\n",
        )
        .unwrap();

        let report = plan_filtered(
            dir.path(),
            ResourceFilter::named(ResourceSelector::Skills, "review"),
            SourceAlias::Claude,
            &[Agent::Codex],
        )
        .unwrap();

        assert!(report
            .actions
            .iter()
            .any(|action| action.path == Path::new(".codex/skills/review/SKILL.md")));
        assert!(!report
            .actions
            .iter()
            .any(|action| action.path == Path::new(".codex/skills/lint/SKILL.md")));
    }

    #[test]
    fn non_utf8_skill_assets_are_blocked() {
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

        let report = plan(
            dir.path(),
            ResourceSelector::Skills,
            SourceAlias::Claude,
            &[Agent::Codex],
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert_eq!(report.actions[0].reason, "non-portable");
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not UTF-8")));
        assert!(write_plan(dir.path(), &report).is_err());
        assert!(!dir.path().join(".codex/skills/review/SKILL.md").exists());
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
        assert_eq!(report.actions[0].action, PlanActionKind::Update);
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
    fn unified_diff_uses_hunks_and_context() {
        let old = (1..=9)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let new = old.replace("line 5", "changed 5");

        let diff = unified_diff(Path::new("CLAUDE.md"), Some(old), &new);

        assert!(diff.starts_with("--- a/CLAUDE.md\n+++ b/CLAUDE.md\n"));
        assert!(diff.contains("@@ -2,7 +2,7 @@\n"));
        assert!(!diff.contains(" line 1\n"));
        assert!(!diff.contains(" line 9\n"));
        assert!(diff.contains("-line 5\n"));
        assert!(diff.contains("+changed 5\n"));
    }

    #[test]
    fn unified_diff_create_file_has_zero_old_count() {
        let diff = unified_diff(Path::new("CLAUDE.md"), None, "one\ntwo\n");

        assert!(diff.contains("@@ -0,0 +1,2 @@\n"));
        assert!(diff.contains("+one\n"));
        assert!(diff.contains("+two\n"));
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

    #[test]
    fn portable_subagent_plan_renders_codex_toml() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
        fs::write(
            dir.path().join(".claude/agents/reviewer.md"),
            "---\nname: reviewer\n---\nReview carefully.\n",
        )
        .unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Subagents,
            SourceAlias::Claude,
            &[Agent::Codex],
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Create);
        assert_eq!(report.actions[0].resource_id, "subagents:reviewer");
        let rendered = report.actions[0].rendered.as_ref().unwrap();
        assert_eq!(rendered.path, Path::new(".codex/agents/reviewer.toml"));
        assert!(rendered.contents.contains("name = \"reviewer\""));
        assert!(rendered.contents.contains("instructions = "));
        assert!(rendered.contents.contains("Review carefully."));
    }

    #[test]
    fn portable_subagent_plan_renders_markdown_targets() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
        fs::write(
            dir.path().join(".claude/agents/reviewer.md"),
            "---\nname: reviewer\ndescription: Review code\nmodel: sonnet\n---\nReview carefully.\n",
        )
        .unwrap();

        let opencode_report = plan(
            dir.path(),
            ResourceSelector::Subagents,
            SourceAlias::Claude,
            &[Agent::OpenCode],
        )
        .unwrap();

        assert!(opencode_report.actions.iter().any(|action| {
            action.action == PlanActionKind::Create
                && action.path == Path::new(".opencode/agents/reviewer.md")
                && action
                    .rendered
                    .as_ref()
                    .unwrap()
                    .contents
                    .contains("model: sonnet\n")
        }));
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".opencode/agents")).unwrap();
        fs::write(
            dir.path().join(".opencode/agents/reviewer.md"),
            "---\nname: reviewer\ndescription: Review code\nmodel: sonnet\n---\nReview carefully.\n",
        )
        .unwrap();
        let claude_report = plan(
            dir.path(),
            ResourceSelector::Subagents,
            SourceAlias::OpenCode,
            &[Agent::Claude],
        )
        .unwrap();
        assert!(claude_report.actions.iter().any(|action| {
            action.action == PlanActionKind::Create
                && action.path == Path::new(".claude/agents/reviewer.md")
                && action
                    .rendered
                    .as_ref()
                    .unwrap()
                    .contents
                    .ends_with("Review carefully.\n")
        }));
    }

    #[test]
    fn subagent_with_tool_policy_is_blocked_and_not_rendered() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
        fs::write(
            dir.path().join(".claude/agents/reviewer.md"),
            "---\nname: reviewer\ntools:\n  - Read\n---\nReview carefully.\n",
        )
        .unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Subagents,
            SourceAlias::Claude,
            &[Agent::Codex],
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert_eq!(report.actions[0].resource_id, "subagents:reviewer");
        assert!(report.actions[0].rendered.is_none());
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("subagent.tools: blocked")));
        assert!(write_plan(dir.path(), &report).is_err());
        assert!(!dir.path().join(".agentsync/state.json").exists());
    }

    #[test]
    fn subagent_with_native_only_field_is_blocked_and_not_rendered() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
        fs::write(
            dir.path().join(".claude/agents/reviewer.md"),
            "---\nname: reviewer\nenabled: true\n---\nReview carefully.\n",
        )
        .unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Subagents,
            SourceAlias::Claude,
            &[Agent::Codex],
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("native-only fields")));
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("subagent.native_extensions: blocked")
        }));
    }

    #[test]
    fn subagent_with_native_config_field_is_blocked_and_not_rendered() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
        fs::write(
            dir.path().join(".claude/agents/reviewer.md"),
            "---\nname: reviewer\nconfig:\n  effort: high\n  sandbox: danger-full-access\n---\nReview carefully.\n",
        )
        .unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Subagents,
            SourceAlias::Claude,
            &[Agent::Codex],
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("native-only fields")));
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("subagent.effort: partial")));
        assert!(report.actions[0].rendered.is_none());
    }

    #[test]
    fn subagent_with_path_separator_name_is_blocked_and_not_rendered() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
        fs::write(
            dir.path().join(".claude/agents/reviewer.md"),
            "---\nname: ../escape\n---\nReview carefully.\n",
        )
        .unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Subagents,
            SourceAlias::Claude,
            &[Agent::Codex],
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("path separators")));
        assert!(report.actions[0].rendered.is_none());
    }

    #[test]
    fn command_plan_is_blocked_and_not_rendered() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".opencode/commands")).unwrap();
        fs::write(dir.path().join(".opencode/commands/deploy.md"), "deploy\n").unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Commands,
            SourceAlias::OpenCode,
            &[Agent::Codex],
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert_eq!(
            report.actions[0].resource_id,
            "commands:opencode:.opencode/commands/deploy.md"
        );
        assert!(report.actions[0].rendered.is_none());
        assert!(write_plan(dir.path(), &report).is_err());
        assert!(!dir.path().join(".agentsync/state.json").exists());
    }

    #[test]
    fn prompt_only_command_plan_renders_opencode_markdown() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("opencode.json"),
            r#"{"command":{"deploy":{"template":"Deploy the app","description":"Deploy","model":"anthropic/claude-sonnet-4-5"}}}"#,
        )
        .unwrap();

        let report = plan_filtered(
            dir.path(),
            ResourceFilter::named(ResourceSelector::Commands, "deploy"),
            SourceAlias::OpenCode,
            &[Agent::OpenCode],
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Create);
        assert_eq!(
            report.actions[0].resource_id,
            "commands:opencode:opencode.json:deploy"
        );
        let rendered = report.actions[0].rendered.as_ref().unwrap();
        assert_eq!(rendered.path, Path::new(".opencode/commands/deploy.md"));
        assert!(rendered.contents.contains("description: Deploy\n"));
        assert!(rendered
            .contents
            .contains("model: anthropic/claude-sonnet-4-5\n"));
        assert!(rendered.contents.ends_with("Deploy the app"));
    }

    #[test]
    fn command_with_shell_output_is_blocked_and_not_rendered() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".opencode/commands")).unwrap();
        fs::write(
            dir.path().join(".opencode/commands/deploy.md"),
            "deploy with !`npm run build`\n",
        )
        .unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Commands,
            SourceAlias::OpenCode,
            &[Agent::OpenCode],
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("shell output")));
        assert!(report.actions[0].rendered.is_none());
    }

    #[test]
    fn command_with_agent_execution_behavior_is_blocked_and_not_rendered() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".opencode/commands")).unwrap();
        fs::write(
            dir.path().join(".opencode/commands/deploy.md"),
            "---\nagent: build\n---\nDeploy the app\n",
        )
        .unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Commands,
            SourceAlias::OpenCode,
            &[Agent::OpenCode],
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("command.agent: blocked")));
        assert!(report.actions[0].rendered.is_none());
    }

    #[test]
    fn hook_plan_is_blocked_and_not_rendered() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".claude")).unwrap();
        fs::write(
            dir.path().join(".claude/settings.json"),
            r#"{"hooks":{"PreToolUse":[]}}"#,
        )
        .unwrap();

        let report = plan(
            dir.path(),
            ResourceSelector::Hooks,
            SourceAlias::Claude,
            &[Agent::Codex],
        )
        .unwrap();

        assert_eq!(report.actions[0].action, PlanActionKind::Block);
        assert_eq!(
            report.actions[0].resource_id,
            "hooks:claude:.claude/settings.json"
        );
        assert!(report.actions[0].rendered.is_none());
        assert!(write_plan(dir.path(), &report).is_err());
        assert!(!dir.path().join(".agentsync/state.json").exists());
    }
}
