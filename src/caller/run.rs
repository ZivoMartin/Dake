//! # Caller Module
//!
//! This module is responsible for initiating a build request from the user's
//! current directory. It parses the Makefile, generates distributed makefiles,
//! contacts the daemon, and waits for build completion.
//!
//! This module acts as the entrypoint for distributed builds when the user
//! executes `dake <make-args>`.

use std::{env::current_dir, fs::write};

use crate::{
    caller::{fetch_id::fetch_fresh_id, start::start},
    lexer::guess_path_and_lex,
    makefile::RemoteMakefileSet,
    network::{connect_with_daemon_or_start_it, get_daemon_tcp_sock, get_daemon_unix_sock},
    process_id::ProjectId,
    utils::get_dake_path,
};
use anyhow::{Context, Result};
use tokio::fs::remove_file;
use tracing::info;

/// Name of the temporary makefile generated for the local build.
const TMP_MAKEFILE_NAME: &'static str = "dake_tmp_makefile";

/// Initiates a distributed build request.
#[tracing::instrument]
pub async fn make(mut args: Vec<String>) -> Result<i32> {
    let daemon_unix_sock = get_daemon_unix_sock()?;
    info!("Fetched daemon_unix_sock successfully: {daemon_unix_sock}");
    let daemon_tcp_sock = get_daemon_tcp_sock()?;
    info!("Fetched daemon_tcp_sock successfully: {daemon_tcp_sock}");

    let caller_dir = current_dir()?;
    info!("Caller started in directory: {:?}", caller_dir);

    // Step 1: Lexing makefile
    info!("Lexing makefile..");
    let tokens = guess_path_and_lex()?;
    info!("Successfully lexed Makefile into {} tokens", tokens.len());

    // Step 2: Connecting with daemon
    info!("Connecting to the daemon from the caller...");
    let mut stream = connect_with_daemon_or_start_it(daemon_unix_sock).await?;
    info!("Connected to the daemon successfully.");

    // Step 3: Fetch a fresh process id
    let project_id = ProjectId::new(daemon_tcp_sock, caller_dir.clone());

    info!("Fetching pid for project {project_id:?}.");
    let pid = fetch_fresh_id(&mut stream, project_id).await?;

    // Step 4: Generate makefiles
    let makefiles = RemoteMakefileSet::generate(tokens, pid.clone(), get_dake_path()?);
    info!("Generated RemoteMakefileSet for daemon");

    write(TMP_MAKEFILE_NAME, makefiles.my_makefile())
        .context("Failed to write temporary dake makefile")?;
    info!("Temporary makefile `{}` written", TMP_MAKEFILE_NAME);

    // Step 5: Modifying arguments
    args.append(&mut vec![
        String::from("--file"),
        String::from(TMP_MAKEFILE_NAME),
    ]);
    info!("Arguments for make prepared: {:?}", args);

    // Step 6: Starting the process.
    let exit_code = start(&mut stream, pid, makefiles, args).await?;

    remove_file(TMP_MAKEFILE_NAME)
        .await
        .context("Failed to remove tmp makefile at the end of the process.")?;

    info!("Caller finished execution");
    Ok(exit_code)
}
