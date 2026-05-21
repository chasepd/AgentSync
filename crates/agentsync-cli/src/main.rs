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
        #[arg(long, default_value_t = CliScope::Project)]
        scope: CliScope,

        #[arg(long)]
        json: bool,
    },
    /// Show missing formats and drift.
    Status {
        #[arg(long, default_value_t = CliScope::Project)]
        scope: CliScope,

        #[arg(long)]
        check: bool,

        #[arg(long)]
        json: bool,
    },
    /// Preview generated target changes.
    Diff {
        resource: CliResource,

        #[arg(long)]
        from: CliSource,

        #[arg(long, value_delimiter = ',')]
        to: Vec<CliAgent>,

        #[arg(long)]
        json: bool,
    },
    /// Generate or update target formats. Writes require --write.
    Sync {
        resource: CliResource,

        #[arg(long)]
        from: CliSource,

        #[arg(long, value_delimiter = ',')]
        to: Vec<CliAgent>,

        #[arg(long)]
        dry_run: bool,

        #[arg(long)]
        write: bool,

        #[arg(long)]
        json: bool,
    },
    /// Create an AgentSync config file.
    Init,
    /// Validate local setup and compatibility.
    Doctor,
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
            let scope: Scope = scope.into();
            let report = agentsync_core::scan(scope)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", report.to_table());
            }
        }
        Command::Status { scope, check, json } => {
            let scope: Scope = scope.into();
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
            let targets = to.into_iter().map(Agent::from).collect::<Vec<_>>();
            let report = agentsync_core::plan(
                std::env::current_dir()?,
                resource.into(),
                from.into(),
                &targets,
            )?;
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
            let targets = to.into_iter().map(Agent::from).collect::<Vec<_>>();
            let report = agentsync_core::plan(
                std::env::current_dir()?,
                resource.into(),
                from.into(),
                &targets,
            )?;
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
        Command::Init => {
            println!("init is not implemented yet");
        }
        Command::Doctor => {
            println!("doctor is not implemented yet");
        }
    }

    Ok(())
}
