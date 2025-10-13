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

use std::{env::current_dir, fs::write, time::Duration};

use crate::{
    lexer::guess_path_and_lex, makefile::RemoteMakefileSet, network::Message, process_id::ProcessId,
};
use anyhow::{Context, Result};
use tokio::{net::TcpListener, time::timeout};
use tracing::{error, info, warn};

use crate::{
    dec,
    network::{
        DaemonMessage, MessageKind, ProcessMessage, contact_daemon_or_start_it, get_daemon_sock,
        read_next_message,
    },
};

/// Name of the temporary makefile generated for the local build.
const TMP_MAKEFILE_NAME: &'static str = "dake_tmp_makefile";

/// Initiates a distributed build request.
pub async fn make(mut args: Vec<String>) -> Result<u32> {
    // Step 1: get caller working directory
    let caller_dir = current_dir()?;
    info!("Caller started in directory: {:?}", caller_dir);

    // Step 2: parse and lex Makefile
    let tokens = guess_path_and_lex()?;
    info!("Successfully lexed Makefile into {} tokens", tokens.len());

    // Step 3: generate distributed makefiles
    let makefiles = RemoteMakefileSet::generate(tokens, caller_dir.clone(), get_daemon_sock());
    let daemon_sock = get_daemon_sock();
    info!("Generated RemoteMakefileSet for daemon at {}", daemon_sock);

    // Step 4: write local temporary makefile
    write(TMP_MAKEFILE_NAME, makefiles.my_makefile())
        .context("Failed to write temporary dake makefile")?;
    info!("Temporary makefile `{}` written", TMP_MAKEFILE_NAME);

    // Step 5: append temp makefile to arguments
    args.append(&mut vec![
        String::from("--file"),
        String::from(TMP_MAKEFILE_NAME),
    ]);
    info!("Arguments for make prepared: {:?}", args);

    // Step 6: start a local listener for daemon callbacks
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("When starting the caller socket.")?;
    let caller_addr = listener
        .local_addr()
        .context("When requesting the caller socket address")?;
    info!("Caller listener bound at {}", caller_addr);

    // Step 7: create NewProcess message
    let message = Message::new(
        DaemonMessage::NewProcess {
            makefiles: makefiles.drop_makefiles(),
            args,
        },
        ProcessId::new(daemon_sock, caller_dir),
        caller_addr,
    );
    info!("Sending NewProcess message to daemon at {}", daemon_sock);

    // Step 8: send message to daemon (starting it if necessary)
    contact_daemon_or_start_it(message).await?;
    info!("NewProcess message delivered successfully");

    // Step 9: wait for daemon response with a 1 second timeout
    let timer = timeout(Duration::from_secs(1), async move {
        let daemon_addr = daemon_sock.ip();
        info!("Caller is waiting for daemon response from {}", daemon_addr);

        loop {
            match listener.accept().await {
                Ok((tcp_stream, addr)) => {
                    let ip = addr.ip();
                    if ip == daemon_addr {
                        info!("Accepted connection from daemon at {}", ip);
                        break tcp_stream;
                    } else {
                        warn!(
                            "Unexpected connection received: expected {}, got {}",
                            daemon_addr, ip
                        );
                    }
                }
                Err(e) => {
                    warn!("The listener failed to accept a connection: {e}");
                }
            }
        }
    });

    // Step 10: process daemon messages if connection succeeded
    let exit_code = if let Ok(mut tcp_stream) = timer.await {
        info!("Caller connected to daemon stream, awaiting messages...");

        loop {
            // Read next message from daemon
            let msg = match read_next_message(&mut tcp_stream, MessageKind::ProcessMessage).await {
                Ok(Some(msg)) => msg,
                Ok(None) => {
                    error!("Caller connection closed naturally; expected closure via End message.");
                    break 1;
                }
                Err(e) => {
                    warn!("Failed to receive a message from the daemon: {e}");
                    continue;
                }
            };

            // Deserialize into ProcessMessage
            let msg: ProcessMessage = dec!(msg)?;
            match msg {
                ProcessMessage::End { exit_code } => {
                    info!("Caller received End message from daemon, build completed");
                    break exit_code;
                }
                ProcessMessage::StdoutLog { log } => print!("{log}"),
                ProcessMessage::StderrLog { log } => eprint!("{log}"),
            }
        }
    } else {
        error!("The daemon did not respond within timeout");
        1
    };

    info!("Caller finished execution");
    Ok(exit_code)
}
