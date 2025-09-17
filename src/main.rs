mod caller;
mod deamon;
mod fetch;

use std::env::args;

use anyhow::Result;

fn main() -> Result<()> {
    let mut args = args().skip(1);
    match args.next().as_deref() {
        Some("fetch") => fetch::fetch(),
        Some("deamon") => deamon::start(),
        _ => caller::make(args.collect())?,
    }
    Ok(())
}
