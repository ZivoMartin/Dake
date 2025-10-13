//! # Daemon Listener
//!
//! This module defines the entrypoint for the **Dake daemon**.  
//!
//! The daemon is responsible for:
//! - Accepting incoming TCP connections on the daemon socket.
//! - Reading and deserializing [`DaemonMessage`]s sent by callers or distributors.
//! - Dispatching requests to the appropriate handler:
//!   - [`new_process`] for starting a new build process
//!   - [`receiv_makefile`] for receiving and storing distributed makefiles
//!   - [`handle_fetch`] for handling fetch requests (artifacts)
//!
//! The daemon runs indefinitely, spawning tasks to handle each connection
//! asynchronously.

use anyhow::{Context, Result};
use tokio::{net::TcpListener, task::spawn};
use tracing::{info, warn};

use crate::{
    dec,
    network::{
        Message, MessageKind,
        fs::init_fs,
        get_daemon_sock,
        handlers::{
            OutputFile, handle_error, handle_fetch, handle_log, new_process, receiv_makefile,
        },
        message_ctx::MessageCtx,
        messages::DaemonMessage,
        state::State,
        utils::read_next_message,
    },
};

/// Starts the daemon listener.
/// For each incoming [`DaemonMessage`], a new task is spawned to run the
/// corresponding handler.
pub async fn start() -> Result<()> {
    // Initialize filesystem structure before starting daemon
    init_fs()?;
    info!("Daemon filesystem initialized");

    // Bind the daemon listener socket
    let listener = TcpListener::bind(get_daemon_sock())
        .await
        .context("When starting the daemon.")?;
    info!("Daemon started and listening on {}", get_daemon_sock());

    let state = State::new();

    // Main accept loop: handle new TCP connections
    loop {
        let (mut tcp_stream, addr) = match listener.accept().await {
            Ok((tcp_stream, addr)) => {
                info!("Daemon accepted new connection from {}", addr);
                (tcp_stream, addr)
            }
            Err(e) => {
                warn!("Daemon failed to accept connection: {}", e);
                continue;
            }
        };

        // Spawn a task for this connection
        let state = state.clone();
        spawn(async move {
            info!("Daemon spawned task to handle connection from {}", addr);

            loop {
                // Read next DaemonMessage from this TCP stream
                let message =
                    match read_next_message(&mut tcp_stream, MessageKind::DaemonMessage).await {
                        Ok(Some(msg)) => {
                            info!("Daemon received raw DaemonMessage from {}", addr);
                            msg
                        }
                        Ok(None) => {
                            warn!("Connection {} closed by peer", addr);
                            break;
                        }
                        Err(e) => {
                            warn!(
                                "Failed to read DaemonMessage from {}: {}",
                                addr,
                                e.root_cause()
                            );
                            break;
                        }
                    };

                // Attempt to deserialize the DaemonMessage
                let message: Message<DaemonMessage> = match dec!(message) {
                    Ok(msg) => {
                        info!("Successfully decoded DaemonMessage from {}", addr);
                        msg
                    }
                    Err(_) => {
                        warn!("Failed to decrypt DaemonMessage from {}", addr);
                        break;
                    }
                };

                // Spawn another task for handling the specific message
                let state = state.clone();
                spawn(async move {
                    let pid = message.pid.clone();
                    let client = message.client;
                    let ctx = MessageCtx::new(state, pid.clone(), client);
                    match message.inner {
                        DaemonMessage::NewProcess { makefiles, args } => {
                            info!(
                                "Handling NewProcess request from pid {:?}, client {}",
                                pid, client
                            );
                            new_process(ctx, makefiles, args).await
                        }
                        DaemonMessage::NewMakefile {
                            makefile,
                            process_datas,
                        } => {
                            info!(
                                "Handling Distribute request from pid {:?}, client {}",
                                pid, client
                            );
                            receiv_makefile(ctx, makefile, process_datas).await
                        }
                        DaemonMessage::Fetch {
                            target,
                            labeled_path,
                        } => {
                            info!(
                                "Handling Fetch request for target '{}' from pid {:?}, client {}",
                                target, pid, client
                            );
                            handle_fetch(ctx, target, labeled_path).await
                        }
                        DaemonMessage::StdoutLog { log } => {
                            info!("Handling new log from pid {pid:?}, client {client}");
                            handle_log(ctx, log, OutputFile::Stdout).await
                        }
                        DaemonMessage::StderrLog { log } => {
                            info!("Handling new err from pid {pid:?}, client {client}");
                            handle_log(ctx, log, OutputFile::Stderr).await
                        }
                        DaemonMessage::MakeError {
                            guilty_node,
                            exit_code,
                        } => {
                            info!(
                                "The process {pid:?} failed with exit code {exit_code} on {guilty_node}."
                            );
                            handle_error(ctx, guilty_node, exit_code).await
                        }
                        DaemonMessage::Done => todo!(),
                    }
                });
            }
            info!("Daemon task for {} terminated", addr);
        });
    }
}
