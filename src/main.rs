mod caller;
mod macros;
mod network;

use std::env::args;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(e) = simple_logger::init() {
        eprintln!("Failed to init the logger, no warning messages will be printed: {e}");
    }

    let mut args = args().skip(1);
    match args.next().as_deref() {
        Some("fetch") => network::fetch(),
        Some("deamon") => network::start().await?,
        _ => caller::make(args.collect()).await?,
    }
    Ok(())
}
