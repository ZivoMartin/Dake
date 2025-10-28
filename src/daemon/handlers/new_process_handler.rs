//! # New Process Handler
//!
//! Handles incoming [`DaemonMessage::NewProcess`] messages.
//!
//! ## Responsibilities
//! - Distribute remote makefiles to the necessary hosts via [`distribute`]
//! - Register process metadata in the shared [`State`]
//! - Spawn and monitor a local `make` process
//! - Stream logs and final results back to the caller
//! - Forward termination or error notifications to all involved hosts
//!
//! ## Behavior
//! - If distribution fails, error forwarding is **not yet implemented**
//! - The function runs until the local process completes or a `Notif::Error` is received

use crate::{
    daemon::{
        MessageCtx, Notif, broadcast_done, distribute, execute_make, handlers::OutputFile,
        process_datas::ProcessDatas,
    },
    makefile::RemoteMakefile,
    network::{Message, ProcessMessage, write_message},
};
use tokio::select;
use tracing::{debug, error, info, warn};

/// Handles the creation and supervision of a new distributed `make` process.
///
/// # Workflow
/// 1. Distributes makefiles to remote daemons.
/// 2. Registers process metadata in the shared state.
/// 3. Spawns and monitors the local `make` process.
/// 4. Forwards logs and handles error/cancel notifications.
/// 5. Sends a final [`ProcessMessage::End`] to the originating client.
#[tracing::instrument]
pub async fn new_process<'a>(
    MessageCtx { state, pid, stream }: MessageCtx<'a>,
    makefiles: Vec<RemoteMakefile>,
    args: Vec<String>,
) {
    info!("Starting new process handler");

    let daemon_addr = state.daemon_sock.clone();

    if daemon_addr != pid.sock() {
        error!("Mismatched sockets");
        return;
    }

    // --- Step 1: Distribute remote makefiles ---
    let involved_hosts: Vec<_> = makefiles.iter().map(|m| m.sock()).cloned().collect();

    info!("Distributing makefiles to involved hosts");
    let process_datas = ProcessDatas::new(daemon_addr, involved_hosts.clone(), args.clone());

    match distribute(&state, pid.clone(), makefiles, process_datas.clone()).await {
        Ok(_) => info!(?pid, "Makefiles successfully distributed"),
        Err(e) => {
            let msg = ProcessMessage::StderrLog {
                log: format!("Dake failed to distribute makefile to remote hosts: {e:?}"),
            };

            if let Err(e) = write_message(stream, Message::new(msg, pid.clone())).await {
                warn!(?pid, error=?e, "Failed to forward distribute error to client");
            }

            let msg = ProcessMessage::End { exit_code: 1 };

            if let Err(e) = write_message(stream, Message::new(msg, pid.clone())).await {
                warn!(?pid, error=?e, "Failed to send end message to client");
            }
            return;
        }
    }

    // --- Step 2: Register process in shared state ---
    state.register_process(pid.clone(), process_datas).await;
    debug!(?pid, "Registered process in shared database");

    // --- Step 3: Execute local make process ---
    info!(?pid, args = ?args, dir = ?pid.path(), "Launching local make process");

    let hub = state.notifier_hub().clone();
    let mut subscriber = hub.lock().await.subscribe(&pid, 100);

    // Exit code placeholder — filled either by make completion or notification
    let exit_code = loop {
        select! {
            // Handle local make process completion
            result = execute_make(&state, pid.clone(), pid.path().clone(), None, &args) => {
                break match result {
                    Ok(Some(status)) => {
                        if status.success() {
                            info!(?pid, "Make process completed successfully");
                        } else {
                            warn!(?pid, code=?status.code(), "Make exited with non-zero code");
                        }
                        status.code().unwrap_or(1)
                    }
                    Ok(None) => {
                        warn!(?pid, "Make process aborted unexpectedly (received None)");
                        1
                    }
                    Err(e) => {
                        error!(?pid, error=?e, "Failed to execute make process");
                        1
                    }
                }
            }

            // Handle remote notifications from other daemons
            notif = subscriber.recv() => {
                let notif = match notif {
                    Some(n) => n,
                    None => {
                        warn!(?pid, "Notification stream closed unexpectedly");
                        break 1;
                    }
                };

                match notif.as_ref() {
                    Notif::Error { exit_code, guilty_node } => {
                        error!(
                            ?pid,
                            guilty = %guilty_node,
                            exit_code = *exit_code,
                            "Received distributed error, aborting"
                        );
                        if let Err(e) = broadcast_done(&state, pid.clone()).await {
                            warn!(?pid, error=?e, "Failed to broadcast Done message");
                        }
                        break *exit_code;
                    }
                    Notif::Log { output, log } => {
                        debug!(?pid, output=?output, "Forwarding log to client");
                        let msg = match output {
                            OutputFile::Stdout => ProcessMessage::StdoutLog { log: log.to_string() },
                            OutputFile::Stderr => ProcessMessage::StderrLog { log: log.to_string() },
                        };
                        if let Err(e) = write_message(stream, Message::new(msg, pid.clone())).await {
                            warn!(?pid, error=?e, "Failed to forward log to client");
                        }
                    }
                    _ => {
                        debug!(?pid, notif=?notif, "Ignoring irrelevant notification");
                        continue;
                    }
                }
            }
        }
    };

    // --- Step 4: Send final End message ---
    let end_message = Message::new(ProcessMessage::End { exit_code }, pid.clone());

    match write_message(stream, end_message).await {
        Ok(_) => info!("Sent End message to caller"),
        Err(e) => warn!("Failed to send End message: {e}"),
    }

    info!(?pid, "NewProcess handler completed");
}
