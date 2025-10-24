//! # Caller Module
//!
//! This module is responsible for initiating a build request from the user's
//! current directory. It parses the Makefile, generates distributed makefiles,
//! contacts the daemon, and waits for build completion.
//!
//! The caller:
//! - Parses and rewrites the local Makefile into a temporary `dake_tmp_makefile`
//! - Connects to the daemon, sending a [`DaemonMessage::NewProcess`] request
//! - Waits for a [`ProcessMessage::End`] from the daemon before returning
//!
//! This module acts as the entrypoint for distributed builds when the user
//! executes `dake <make-args>`.

use std::{env::current_dir, fs::write};

use anyhow::{Context, Result};
use tokio::{fs::remove_file, net::TcpListener};
use tracing::info;

use crate::{
    caller::{fetch_id::fetch_fresh_id, start::start},
    daemon::communication::get_daemon_sock,
    lexer::guess_path_and_lex,
    makefile::RemoteMakefileSet,
    process_id::ProjectId,
    utils::get_dake_path,
};

/// Name of the temporary makefile generated for the local build.
const TMP_MAKEFILE_NAME: &'static str = "dake_tmp_makefile";

/// Initiates a distributed build request.
#[tracing::instrument]
pub async fn make(mut args: Vec<String>) -> Result<i32> {
    let daemon_sock = get_daemon_sock()?;
    let caller_dir = current_dir()?;
    info!("Caller started in directory: {:?}", caller_dir);

    // Step 1: Lexing makefile
    info!("Lexing makefile..");
    let tokens = guess_path_and_lex()?;
    info!("Successfully lexed Makefile into {} tokens", tokens.len());

    // Step 2: Init TCP
    info!("Connecting on TCP");
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("When starting the caller socket.")?;

    // Step 3: Fetch a fresh process id
    let project_id = ProjectId::new(daemon_sock, caller_dir.clone());

    info!("Fetching pid for project {project_id:?}.");
    let pid = fetch_fresh_id(&listener, project_id, daemon_sock).await?;

    // Step 4: Generate makefiles
    let makefiles = RemoteMakefileSet::generate(tokens, pid.clone(), get_dake_path()?);
    info!("Generated RemoteMakefileSet for daemon at {}", daemon_sock);

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
    let exit_code = start(&listener, pid, makefiles, args, daemon_sock).await?;

    remove_file(TMP_MAKEFILE_NAME)
        .await
        .context("Failed to remove tmp makefile at the end of the process.")?;

    info!("Caller finished execution");
    Ok(exit_code)
}
