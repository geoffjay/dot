//! dot binary: CLI commands.

mod backup_cmd;
mod diff_cmd;
mod init_cmd;
mod propose_cmd;
mod restore_cmd;
mod update_cmd;
mod write_cmd;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "dot",
    version,
    about = "Modular, versioned dotfile manager",
    disable_help_subcommand = true
)]
struct Cli {
    /// Path to the config file (default: ~/.config/dot/config.toml)
    #[arg(long, global = true)]
    config: Option<String>,

    /// Limit the command to these modules (repeatable)
    #[arg(long, global = true)]
    module: Vec<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Interactively create the dot config
    Init {
        /// Overwrite an existing config
        #[arg(long)]
        force: bool,

        /// Do not prompt; require --answers
        #[arg(long)]
        non_interactive: bool,

        /// TOML file with wizard answers (non-interactive mode)
        #[arg(long, requires = "non_interactive")]
        answers: Option<String>,
    },
    /// Render and install configuration for enabled modules
    Write {
        /// Rewrite files even when unchanged
        #[arg(long)]
        force: bool,
    },
    /// Show what `dot write` would change
    Diff,
    /// Snapshot managed files into the state dir
    Backup {
        /// Snapshots to keep (default 10)
        #[arg(long, default_value_t = 10)]
        keep: usize,
    },
    /// Restore managed files from a backup
    Restore {
        /// Backup id (default: latest)
        #[arg(long)]
        backup: Option<String>,

        /// Overwrite files changed on disk since the backup
        #[arg(long)]
        force: bool,

        /// List available backups instead of restoring
        #[arg(long)]
        list: bool,
    },
    /// Update dot via its install channel (brew, AUR)
    Update {
        /// Report current vs. latest published release without installing
        #[arg(long)]
        check: bool,
    },
    /// Open a GitHub issue with diffs of local changes
    Propose {
        /// Print the issue body without creating it
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Command::Init {
            force,
            non_interactive,
            answers,
        } => init_cmd::run(force, non_interactive, answers),
        Command::Write { force } => write_cmd::run(&cli.module, force),
        Command::Diff => diff_cmd::run(&cli.module),
        Command::Backup { keep } => backup_cmd::run_backup(&cli.module, keep),
        Command::Restore {
            backup,
            force,
            list,
        } => restore_cmd::run(backup, force, list),
        Command::Update { check } => update_cmd::run(check),
        Command::Propose { dry_run } => propose_cmd::run(&cli.module, dry_run),
    };

    if let Err(err) = result {
        // diff(1) convention: exit 2 on trouble. dot diff reports real
        // differences via exit code 1 inside diff_cmd.
        eprintln!("dot: {err}");
        std::process::exit(2);
    }
}

/// Shared helpers for commands.
pub(crate) fn home_dir() -> Result<std::path::PathBuf> {
    std::env::var("HOME")
        .context("$HOME is not set")
        .map(std::path::PathBuf::from)
}

pub(crate) fn load_context(
    modules: &[String],
) -> Result<(
    std::path::PathBuf,
    dot_core::Config,
    dot_core::HostFacts,
    Vec<dot_core::Module>,
)> {
    let home = home_dir()?;
    let config_path = dot_core::Config::resolve_path(None, &home);
    let config = dot_core::Config::load(&config_path)?;

    let facts = dot_core::facts::detect();

    let selected: Vec<dot_core::Module> = if modules.is_empty() {
        dot_core::Module::all()
            .into_iter()
            .filter(|m| config.module_enabled(&m.name, &facts.hostname))
            .filter(|m| m.supports_os(&facts.os))
            .collect()
    } else {
        modules
            .iter()
            .map(|name| dot_core::Module::by_name(name).map_err(anyhow::Error::from))
            .collect::<anyhow::Result<_>>()?
    };

    Ok((home, config, facts, selected))
}
