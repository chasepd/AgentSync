use agentsync_core::report::PlanReport;
use agentsync_core::{
    Agent, AgentSyncError, ConflictStrategy, PlanConflictChoice, PlanOptions, ResourceFilter,
    ResourceSelector, Scope, SourceAlias,
};
use clap::{Parser, Subcommand, ValueEnum};
use std::io::{self, IsTerminal, Write};

#[derive(Debug, Parser)]
#[command(name = "agentsync")]
#[command(about = "Sync coding-agent configuration across native formats.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Discover native agent configuration.
    Scan {
        #[arg(long)]
        scope: Option<CliScope>,

        #[arg(long)]
        json: bool,

        #[arg(long)]
        format: Option<CliFormat>,
    },
    /// Show missing formats and drift.
    Status {
        #[arg(long)]
        scope: Option<CliScope>,

        #[arg(long)]
        check: bool,

        #[arg(long)]
        json: bool,

        #[arg(long)]
        format: Option<CliFormat>,
    },
    /// Preview generated target changes.
    Diff {
        resource: CliResource,

        name: Option<String>,

        #[arg(long)]
        from: Option<CliSource>,

        #[arg(long, value_delimiter = ',')]
        to: Vec<CliAgent>,

        #[arg(long)]
        no_overwrite: bool,

        #[arg(long)]
        strategy: Option<CliConflictStrategy>,

        #[arg(long)]
        json: bool,

        #[arg(long)]
        format: Option<CliFormat>,
    },
    /// Generate or update target formats. Writes require --write.
    Sync {
        resource: CliResource,

        name: Option<String>,

        #[arg(long)]
        from: Option<CliSource>,

        #[arg(long, value_delimiter = ',')]
        to: Vec<CliAgent>,

        #[arg(long)]
        dry_run: bool,

        #[arg(long)]
        write: bool,

        #[arg(long)]
        no_overwrite: bool,

        #[arg(long)]
        interactive: bool,

        #[arg(long)]
        strategy: Option<CliConflictStrategy>,

        #[arg(long)]
        json: bool,

        #[arg(long)]
        format: Option<CliFormat>,
    },
    /// Create an AgentSync config file. Writes require --write.
    Init {
        #[arg(long)]
        write: bool,

        #[arg(long)]
        json: bool,

        #[arg(long)]
        format: Option<CliFormat>,
    },
    /// Validate local setup and compatibility.
    Doctor {
        #[arg(long)]
        check: bool,

        #[arg(long)]
        json: bool,

        #[arg(long)]
        format: Option<CliFormat>,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum CliScope {
    Project,
    User,
    All,
}

#[derive(Clone, Debug, ValueEnum)]
enum CliResource {
    #[value(alias = "rule")]
    Rules,
    #[value(alias = "skill")]
    Skills,
    #[value(alias = "subagent")]
    Subagents,
    #[value(alias = "command")]
    Commands,
    #[value(alias = "hook")]
    Hooks,
}

#[derive(Clone, Debug, ValueEnum)]
enum CliSource {
    AgentsMd,
    Codex,
    Claude,
    #[value(name = "cursor", alias = "cursor-cli")]
    Cursor,
    Opencode,
}

#[derive(Clone, Debug, ValueEnum)]
enum CliAgent {
    Codex,
    Claude,
    #[value(name = "cursor", alias = "cursor-cli")]
    Cursor,
    Opencode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliFormat {
    Table,
    Json,
}

#[derive(Clone, Debug, ValueEnum)]
enum CliConflictStrategy {
    Source,
    Newest,
}

impl std::fmt::Display for CliScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project => write!(f, "project"),
            Self::User => write!(f, "user"),
            Self::All => write!(f, "all"),
        }
    }
}

impl From<CliScope> for Scope {
    fn from(value: CliScope) -> Self {
        match value {
            CliScope::Project => Scope::Project,
            CliScope::User => Scope::User,
            CliScope::All => Scope::All,
        }
    }
}

impl From<CliResource> for ResourceSelector {
    fn from(value: CliResource) -> Self {
        match value {
            CliResource::Rules => Self::Rules,
            CliResource::Skills => Self::Skills,
            CliResource::Subagents => Self::Subagents,
            CliResource::Commands => Self::Commands,
            CliResource::Hooks => Self::Hooks,
        }
    }
}

impl From<CliSource> for SourceAlias {
    fn from(value: CliSource) -> Self {
        match value {
            CliSource::AgentsMd => Self::AgentsMd,
            CliSource::Codex => Self::Codex,
            CliSource::Claude => Self::Claude,
            CliSource::Cursor => Self::CursorCli,
            CliSource::Opencode => Self::OpenCode,
        }
    }
}

impl From<CliAgent> for Agent {
    fn from(value: CliAgent) -> Self {
        match value {
            CliAgent::Codex => Self::Codex,
            CliAgent::Claude => Self::Claude,
            CliAgent::Cursor => Self::CursorCli,
            CliAgent::Opencode => Self::OpenCode,
        }
    }
}

impl From<CliConflictStrategy> for ConflictStrategy {
    fn from(value: CliConflictStrategy) -> Self {
        match value {
            CliConflictStrategy::Source => Self::Source,
            CliConflictStrategy::Newest => Self::Newest,
        }
    }
}

fn main() -> Result<(), AgentSyncError> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan {
            scope,
            json,
            format,
        } => {
            let output = resolve_output_format(json, format);
            let scope = resolve_scope(scope)?;
            let report = agentsync_core::scan(scope)?;
            if output == CliFormat::Json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", report.to_table());
            }
        }
        Command::Status {
            scope,
            check,
            json,
            format,
        } => {
            let output = resolve_output_format(json, format);
            let scope = resolve_scope(scope)?;
            let report = agentsync_core::status(scope)?;
            if output == CliFormat::Json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", report.to_table());
            }
            if check && report.has_blocking_issues() {
                std::process::exit(1);
            }
        }
        Command::Diff {
            resource,
            name,
            from,
            to,
            no_overwrite,
            strategy,
            json,
            format,
        } => {
            let output = resolve_output_format(json, format);
            let resource: ResourceSelector = resource.into();
            let (from, targets) = resolve_plan_args(resource, from, to)?;
            let report = agentsync_core::plan_filtered_with_options(
                std::env::current_dir()?,
                resource_filter(resource, name),
                from,
                &targets,
                plan_options(no_overwrite, strategy),
            )?;
            if output == CliFormat::Json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report.to_text());
            }
        }
        Command::Sync {
            resource,
            name,
            from,
            to,
            dry_run,
            write,
            no_overwrite,
            interactive,
            strategy,
            json,
            format,
        } => {
            let output = resolve_output_format(json, format);
            if interactive && output == CliFormat::Json {
                return Err(AgentSyncError::InvalidArgument(
                    "sync --interactive cannot be used with JSON output".to_string(),
                ));
            }
            if interactive && !io::stdin().is_terminal() {
                return Err(AgentSyncError::InvalidArgument(
                    "sync --interactive requires a TTY".to_string(),
                ));
            }
            let resource: ResourceSelector = resource.into();
            let (from, targets) = resolve_plan_args(resource, from, to)?;
            let mut report = agentsync_core::plan_filtered_with_options(
                std::env::current_dir()?,
                resource_filter(resource, name),
                from,
                &targets,
                plan_options(no_overwrite, strategy),
            )?;
            if interactive {
                resolve_interactive_conflicts(&mut report)?;
            }
            if output == CliFormat::Json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report.to_text());
            }
            if write {
                agentsync_core::write_plan(std::env::current_dir()?, &report)?;
            } else if !dry_run && output != CliFormat::Json {
                println!("No files written. Re-run with --write to apply changes.");
            }
        }
        Command::Init {
            write,
            json,
            format,
        } => {
            let output = resolve_output_format(json, format);
            let report = agentsync_core::init()?;
            if output == CliFormat::Json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report.to_text());
            }
            if write {
                agentsync_core::write_init(std::env::current_dir()?, &report)?;
                if output != CliFormat::Json {
                    if report.action == agentsync_core::report::InitActionKind::Create {
                        println!("Wrote {}.", report.path.display());
                    } else {
                        println!("No files written.");
                    }
                }
            } else if output != CliFormat::Json {
                println!("No files written. Re-run with --write to create config.");
            }
        }
        Command::Doctor {
            check,
            json,
            format,
        } => {
            let output = resolve_output_format(json, format);
            let report = agentsync_core::doctor()?;
            if output == CliFormat::Json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report.to_text());
            }
            if check && report.has_blocking_issues() {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

fn resolve_output_format(json: bool, format: Option<CliFormat>) -> CliFormat {
    if json {
        CliFormat::Json
    } else {
        format.unwrap_or(CliFormat::Table)
    }
}

fn resolve_scope(scope: Option<CliScope>) -> Result<Scope, AgentSyncError> {
    if let Some(scope) = scope {
        return Ok(scope.into());
    }
    Ok(
        agentsync_core::config::load_config(std::env::current_dir()?)?
            .and_then(|config| config.defaults.scope)
            .unwrap_or(Scope::Project),
    )
}

fn resource_filter(resource: ResourceSelector, name: Option<String>) -> ResourceFilter {
    match name {
        Some(name) => ResourceFilter::named(resource, name),
        None => ResourceFilter::all(resource),
    }
}

fn plan_options(no_overwrite: bool, strategy: Option<CliConflictStrategy>) -> PlanOptions {
    PlanOptions {
        no_overwrite,
        strategy: strategy.map(ConflictStrategy::from).unwrap_or_default(),
    }
}

fn resolve_interactive_conflicts(report: &mut PlanReport) -> Result<(), AgentSyncError> {
    let conflict_indexes = report
        .actions
        .iter()
        .enumerate()
        .filter_map(|(index, action)| {
            agentsync_core::is_interactive_conflict(action).then_some(index)
        })
        .collect::<Vec<_>>();
    for index in conflict_indexes {
        let action = &report.actions[index];
        eprintln!(
            "Conflict: {} {} ({})",
            action.resource_id,
            action.path.display(),
            action.reason
        );
        if let Some(diff) = &action.diff {
            eprintln!("{diff}");
        }
        let choice = read_interactive_choice()?;
        agentsync_core::resolve_plan_conflict(report, index, choice)?;
    }
    Ok(())
}

fn read_interactive_choice() -> Result<PlanConflictChoice, AgentSyncError> {
    loop {
        eprint!("Choose [s]ource, [t]arget, s[k]ip, [a]bort: ");
        io::stderr().flush()?;
        let mut input = String::new();
        let bytes = io::stdin().read_line(&mut input)?;
        if bytes == 0 {
            return Err(AgentSyncError::InvalidArgument(
                "interactive input ended before conflicts were resolved".to_string(),
            ));
        }
        match input.trim().to_ascii_lowercase().as_str() {
            "s" | "source" => return Ok(PlanConflictChoice::Source),
            "t" | "target" => return Ok(PlanConflictChoice::Target),
            "k" | "skip" => return Ok(PlanConflictChoice::Skip),
            "a" | "abort" => {
                return Err(AgentSyncError::InvalidArgument(
                    "interactive sync aborted".to_string(),
                ))
            }
            _ => eprintln!("Enter source, target, skip, or abort."),
        }
    }
}

fn resolve_plan_args(
    resource: ResourceSelector,
    from: Option<CliSource>,
    to: Vec<CliAgent>,
) -> Result<(SourceAlias, Vec<Agent>), AgentSyncError> {
    let needs_config = from.is_none() || to.is_empty();
    let config = if needs_config {
        agentsync_core::config::load_config(std::env::current_dir()?)?
    } else {
        None
    };
    if let Some(config) = &config {
        validate_resource_enabled(resource, config)?;
    }
    let source = if let Some(source) = from {
        SourceAlias::from(source)
    } else {
        config
            .as_ref()
            .and_then(|config| config.defaults.source)
            .ok_or_else(|| {
                AgentSyncError::InvalidArgument(
                    "missing --from and no defaults.source in .agentsync/config.toml".to_string(),
                )
            })?
    };
    let targets = if to.is_empty() {
        config
            .as_ref()
            .map(|config| config.defaults.targets.clone())
            .unwrap_or_default()
    } else {
        to.into_iter().map(Agent::from).collect()
    };
    if targets.is_empty() {
        return Err(AgentSyncError::InvalidArgument(
            "missing --to and no defaults.targets in .agentsync/config.toml".to_string(),
        ));
    }
    Ok((source, targets))
}

fn validate_resource_enabled(
    resource: ResourceSelector,
    config: &agentsync_core::config::ConfigFile,
) -> Result<(), AgentSyncError> {
    let enabled = match resource {
        ResourceSelector::Rules => config.sync.rules.unwrap_or(true),
        ResourceSelector::Skills => config.sync.skills.unwrap_or(true),
        ResourceSelector::Subagents => config.sync.subagents.unwrap_or(true),
        ResourceSelector::Commands => config.sync.commands.unwrap_or(true),
        ResourceSelector::Hooks => config.sync.hooks.unwrap_or(true),
    };
    if enabled {
        Ok(())
    } else {
        Err(AgentSyncError::InvalidArgument(format!(
            "{} sync is disabled in .agentsync/config.toml",
            resource.as_str()
        )))
    }
}
