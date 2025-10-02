use clap::{CommandFactory, Parser, Subcommand};
use dake::{caller, fetch, network, target_label::TargetLabel};

#[derive(Parser, Debug)]
#[command(infer_subcommands = true, allow_external_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Fetch {
        target: String,
        label: TargetLabel,
    },
    Daemon,
    #[command(external_subcommand)]
    Caller(Vec<String>),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(e) = simple_logger::init() {
        eprintln!("Failed to init the logger, no warning messages will be printed: {e}");
    }

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Fetch { target, label }) => {
            fetch::fetch(label, target).await?;
        }
        Some(Commands::Daemon) => {
            network::start().await?;
        }
        Some(Commands::Caller(args)) => {
            caller::make(args).await?;
        }
        None => {
            Cli::command().print_help()?;
        }
    }

    Ok(())
}
