use std::{fs::write, process::Command};

use anyhow::Result;
use dake::lexer::{guess_path_and_lex, Makefile};

const TMP_MAKEFILE_NAME: &'static str = "dake_tmp_makefile";

pub fn make(mut args: Vec<String>) -> Result<()> {
    let makefile: Makefile = guess_path_and_lex()?;
    let makefiles = makefile.generate("my ip".to_string());

    let content = makefiles
        .get(0)
        .map(|m| m.makefile().clone())
        .unwrap_or(String::new());

    write(TMP_MAKEFILE_NAME, content)?;

    args.append(&mut vec![
        String::from("--file"),
        String::from(TMP_MAKEFILE_NAME),
    ]);

    Command::new("make").args(args).status().unwrap(); // We unwrap here since we don't want to handle internal make errors
    Ok(())
}
