use anyhow::Result;
use dake::lexer::{guess_path_and_lex, Makefile};

fn main() -> Result<()> {
    let makefile: Makefile = guess_path_and_lex()?;
    println!("{makefile:?}");
    Ok(())
}
