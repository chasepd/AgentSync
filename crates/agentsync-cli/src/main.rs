use agentsync_core::{AgentSyncError, Scope};
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
    },
    /// Preview generated target changes.
    Diff,
    /// Generate or update target formats. Writes require --write.
    Sync {
        #[arg(long)]
        write: bool,
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

fn main() -> Result<(), AgentSyncError> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan { scope, json } => {
            let scope: Scope = scope.into();
            let report = agentsync_core::scan(scope)?;
            if json {
                println!("{}", report.to_json_placeholder());
            } else {
                println!("{}", report.to_table());
            }
        }
        Command::Status { scope, check } => {
            let scope: Scope = scope.into();
            let report = agentsync_core::status(scope)?;
            println!("{}", report.to_table());
            if check && report.has_drift() {
                std::process::exit(1);
            }
        }
        Command::Diff => {
            println!("diff planning is not implemented yet");
        }
        Command::Sync { write } => {
            if write {
                println!("sync writes are not implemented yet");
            } else {
                println!("sync dry-run planning is not implemented yet");
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

