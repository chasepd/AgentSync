use agentsync_core::{Agent, AgentSyncError, ResourceSelector, Scope, SourceAlias};
use clap::{Parser, Subcommand, ValueEnum};

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
    },
    /// Show missing formats and drift.
    Status {
        #[arg(long)]
        scope: Option<CliScope>,

        #[arg(long)]
        check: bool,

        #[arg(long)]
        json: bool,
    },
    /// Preview generated target changes.
    Diff {
        resource: CliResource,

        #[arg(long)]
        from: Option<CliSource>,

        #[arg(long, value_delimiter = ',')]
        to: Vec<CliAgent>,

        #[arg(long)]
        json: bool,
    },
    /// Generate or update target formats. Writes require --write.
    Sync {
        resource: CliResource,

        #[arg(long)]
        from: Option<CliSource>,

        #[arg(long, value_delimiter = ',')]
        to: Vec<CliAgent>,

        #[arg(long)]
        dry_run: bool,

        #[arg(long)]
        write: bool,

        #[arg(long)]
        json: bool,
    },
    /// Create an AgentSync config file. Writes require --write.
    Init {
        #[arg(long)]
        write: bool,

        #[arg(long)]
        json: bool,
    },
    /// Validate local setup and compatibility.
    Doctor {
        #[arg(long)]
        check: bool,

        #[arg(long)]
        json: bool,
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
    Rules,
    Skills,
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

fn main() -> Result<(), AgentSyncError> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan { scope, json } => {
            let scope = resolve_scope(scope)?;
            let report = agentsync_core::scan(scope)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", report.to_table());
            }
        }
        Command::Status { scope, check, json } => {
            let scope = resolve_scope(scope)?;
            let report = agentsync_core::status(scope)?;
            if json {
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
            from,
            to,
            json,
        } => {
            let (from, targets) = resolve_plan_args(from, to)?;
            let report =
                agentsync_core::plan(std::env::current_dir()?, resource.into(), from, &targets)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report.to_text());
            }
        }
        Command::Sync {
            resource,
            from,
            to,
            dry_run,
            write,
            json,
        } => {
            let (from, targets) = resolve_plan_args(from, to)?;
            let report =
                agentsync_core::plan(std::env::current_dir()?, resource.into(), from, &targets)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report.to_text());
            }
            if write {
                agentsync_core::write_plan(std::env::current_dir()?, &report)?;
            } else if !dry_run && !json {
                println!("No files written. Re-run with --write to apply changes.");
            }
        }
        Command::Init { write, json } => {
            let report = agentsync_core::init()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report.to_text());
            }
            if write {
                agentsync_core::write_init(std::env::current_dir()?, &report)?;
                if !json {
                    if report.action == agentsync_core::report::InitActionKind::Create {
                        println!("Wrote {}.", report.path.display());
                    } else {
                        println!("No files written.");
                    }
                }
            } else if !json {
                println!("No files written. Re-run with --write to create config.");
            }
        }
        Command::Doctor { check, json } => {
            let report = agentsync_core::doctor()?;
            if json {
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

fn resolve_plan_args(
    from: Option<CliSource>,
    to: Vec<CliAgent>,
) -> Result<(SourceAlias, Vec<Agent>), AgentSyncError> {
    let needs_config = from.is_none() || to.is_empty();
    let config = if needs_config {
        agentsync_core::config::load_config(std::env::current_dir()?)?
    } else {
        None
    };
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
