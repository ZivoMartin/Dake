//! # Dake CLI entrypoint
//!
//! This file defines the main command-line interface for the **Dake** distributed
//! build system. It handles user input, parses CLI arguments using `clap`, and
//! dispatches execution to the appropriate subsystem:
//!
//! - **Fetch**: request a build artifact from a remote daemon.
//! - **Daemon**: start the local build daemon that listens for requests.
//! - **Caller**: intercept and run a `make` process, potentially distributed.
//! - **Clean**: clean the dake workspace
//!
//! The CLI also ensures logging is initialized and provides help output if no
//! command is supplied.

use std::{
    path::PathBuf,
    process::{ExitCode, exit},
};

use clap::{Parser, Subcommand};
use dake::{
    caller,
    daemon::{self, fs},
    fetch,
    network::SocketAddr,
};
use tracing::info;

/// CLI root structure used by `clap` for parsing arguments.
///
/// If no subcommand is specified, the arguments are forwarded
/// to the `caller` mode (make interception).
#[derive(Parser, Debug)]
#[command(infer_subcommands = true, allow_external_subcommands = true)]
struct Cli {
    /// Optional Dake subcommand (e.g., daemon, fetch, clean)
    #[command(subcommand)]
    command: Option<Commands>,

    /// Fallback arguments passed directly to the caller
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

/// All supported subcommands for the CLI.
#[derive(Subcommand, Debug)]
enum Commands {
    /// Fetch a target from a remote daemon
    Fetch {
        /// Path of the caller working directory
        caller_path: PathBuf,

        /// Socket of the caller
        caller_sock: SocketAddr,

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

    /// Clean up Dake cache and workspace
    Clean,

    /// Start the Dake daemon
    Daemon,
}

/// Entry point of the application.
///
/// Parses CLI arguments, and dispatches execution
/// to the relevant Dake subsystem (`fetch`, `daemon`, or `caller`).
#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    info!("Parsed CLI arguments: {:?}", cli);

    let exit_code = match cli.command {
        Some(Commands::Fetch {
            target,
            caller_path,
            caller_sock,
            id,
            labeled_path,
            sock,
        }) => {
            info!("Executing Fetch command for target '{target}' with socket {sock}");
            fetch::fetch(target, labeled_path, caller_path, caller_sock, id, sock).await?;
            0
        }

        Some(Commands::Daemon) => {
            info!("Starting daemon...");
            daemon::start().await?;
            0
        }

        Some(Commands::Clean) => {
            info!("Cleaning dake space..");
            fs::clean()?;
            0
        }

        None => {
            info!("Executing default Caller mode with args: {:?}", cli.args);
            let exit_code = caller::make(cli.args).await?;
            exit_code
        }
    };

    info!("Dake CLI execution finished");
    exit(exit_code)
}
