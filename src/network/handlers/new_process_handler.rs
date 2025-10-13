//! # New Process Handler
//!
//! This module defines the logic for handling a [`DaemonMessage::NewProcess`].
//!
//! Responsibilities:
//! - Distribute remote makefiles to the required hosts via [`distribute`].
//! - Spawn a local `make` process with the provided arguments.
//! - Notify the caller process of completion by sending a [`ProcessMessage::End`].
//!
//! If distribution fails, error forwarding to the caller is not yet implemented.

use crate::{
    makefile::RemoteMakefile,
    network::{
        Message, ProcessMessage, distribute, get_daemon_sock, message_ctx::MessageCtx,
        process_datas::ProcessDatas, utils::send_message,
    },
};
use std::process::Command;
use tracing::{error, info, warn};

/// Handles the creation of a new distributed process.
pub async fn new_process(
    MessageCtx { state, client, pid }: MessageCtx,
    makefiles: Vec<RemoteMakefile>,
    args: Vec<String>,
) {
    info!("NewProcess: Starting new process for pid {pid:?} with client {client}");

    let daemon_addr = get_daemon_sock();

    if daemon_addr != pid.sock {
        warn!("The pid sock should match the caller daemon sock.");
        return;
    };

    // Step 1: Distribute remote makefiles
    let involved_hosts = makefiles
        .iter()
        .map(|m| m.sock())
        .copied()
        .collect::<Vec<_>>();

    match distribute(
        pid.clone(),
        makefiles,
        ProcessDatas::new_remote(daemon_addr, involved_hosts.clone()),
    )
    .await
    {
        Ok(_) => {
            info!(
                "NewProcess: Successfully distributed makefiles for pid {:?}",
                pid
            );
        }
        Err(e) => {
            error!(
                "NewProcess: Failed to distribute makefiles for pid {:?}: {e}",
                pid
            );
            todo!("Forward the error to the caller.");
        }
    }

    // Writing process datas in the shared database
    state.register_process(
        pid.clone(),
        ProcessDatas::new_local(daemon_addr, client, involved_hosts),
    );

    // Step 2: Run local `make` process
    info!(
        "NewProcess: Running local `make` with args {:?} in directory {:?}",
        args, pid.path
    );
    match Command::new("make")
        .args(&args)
        .current_dir(pid.path.clone())
        .status()
    {
        Ok(status) => {
            if status.success() {
                info!(
                    "NewProcess: `make` completed successfully for pid {:?}",
                    pid
                );
            } else {
                warn!(
                    "NewProcess: `make` exited with non-zero status {:?} for pid {:?}",
                    status, pid
                );
            }
        }
        Err(e) => {
            error!(
                "NewProcess: Failed to execute `make` for pid {:?}: {e}",
                pid
            );
        }
    }

    // Step 3: Notify caller with End message
    let end_message = Message::new(
        ProcessMessage::End { exit_code: 0 },
        pid.clone(),
        get_daemon_sock(),
    );
    match send_message(end_message, client).await {
        Ok(_) => {
            info!(
                "NewProcess: Sent End message to caller {} for pid {:?}",
                client, pid
            );
        }
        Err(e) => {
            warn!(
                "NewProcess: Failed to send End message to caller {} for pid {:?}: {e}",
                client, pid
            );
        }
    }

    info!("NewProcess: Handler completed for pid {:?}", pid);
}
