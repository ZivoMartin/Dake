//! # Dake CLI entrypoint
//!
//! This file defines the main command-line interface for the **Dake** distributed
//! build system. It handles user input, parses CLI arguments using `clap`, and
//! dispatches execution to the appropriate subsystem:
//!
//! - **Fetch**: request a build artifact from a remote daemon.
//! - **Daemon**: start the local build daemon that listens for requests.
//! - **Caller**: intercept and run a `make` process, potentially distributed.
//!
//! The CLI also ensures logging is initialized and provides help output if no
//! command is supplied.

use std::{
    net::SocketAddr,
    path::PathBuf,
    process::{ExitCode, exit},
};

use clap::{CommandFactory, Parser, Subcommand};
use dake::{caller, daemon, fetch};
use tracing::info;

/// CLI root structure used by `clap` for parsing arguments.
#[derive(Parser, Debug)]
#[command(infer_subcommands = true, allow_external_subcommands = true)]
struct Cli {
    /// Subcommands for Dake
    #[command(subcommand)]
    command: Option<Commands>,
}

/// All supported subcommands for the CLI.
#[derive(Subcommand, Debug)]
enum Commands {
    /// Fetch a target from a remote daemon
    Fetch {
        /// Path of the caller working directory
        caller_path: PathBuf,

        /// Id of the process used in the pid
        id: u64,

        /// Remote daemon socket to fetch from
        sock: SocketAddr,

        /// Optional labeled path to use when fetching
        #[arg(long = "labeled-path")]
        labeled_path: Option<PathBuf>,

        /// The build target to fetch
        target: String,
    },

    /// Start the Dake daemon
    Daemon,

    /// Caller mode: forward arguments to `make` and run the process
    #[command(external_subcommand)]
    Caller(Vec<String>),
}

/// Entry point of the application.
///
/// Parses CLI arguments, and dispatches execution
/// to the relevant Dake subsystem (`fetch`, `daemon`, or `caller`).
#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    // Parse command-line arguments
    let cli = Cli::parse();
    info!("Parsed CLI arguments: {:?}", cli);

    // Match on subcommand and execute the corresponding logic
    let exit_code = match cli.command {
        Some(Commands::Fetch {
            target,
            caller_path,
            id,
            labeled_path,
            sock,
        }) => {
            info!("Executing Fetch command for target '{target}' with socket {sock}");
            fetch::fetch(target, labeled_path, caller_path, id, sock).await?;
            info!("Fetch command completed successfully");
            0
        }

        Some(Commands::Daemon) => {
            info!("Starting daemon...");
            daemon::start().await?;
            info!("Daemon terminated normally");
            0
        }

        Some(Commands::Caller(args)) => {
            info!("Executing Caller command with args: {:?}", args);
            let exit_code = caller::make(args).await?;
            info!("Caller command completed successfully");
            exit_code
        }

        None => {
            Cli::command().print_help()?;
            1
        }
    };
    info!("Dake CLI execution finished");
    exit(exit_code)
}
