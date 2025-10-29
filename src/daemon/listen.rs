//! # Daemon Listener
//!
//! This module defines the entrypoint for the **Dake daemon**.  
//!
//! The daemon is responsible for:
//! - Accepting incoming TCP/Unix connections on the daemon sockets.
//! - Reading and deserializing [`DaemonMessage`]s sent by callers or distributors.
//! - Dispatching requests to the appropriate handler
//!
//! The daemon runs indefinitely, spawning tasks to handle each connection
//! asynchronously.

use std::{
    fs::remove_file,
    net::{IpAddr, Ipv4Addr},
    path::Path,
};

use anyhow::{Context, Result};
use tokio::{
    net::{TcpListener, UnixListener},
    sync::mpsc::channel,
    task::spawn,
    try_join,
};
use tracing::{info, warn};

use crate::{
    daemon::{
        fs::init_fs,
        handlers::{
            OutputFile, handle_done, handle_error, handle_fetch, handle_fresh_request, handle_log,
            new_process, receiv_makefile,
        },
        message_ctx::MessageCtx,
        state::State,
    },
    dec,
    network::{
        DAEMON_UNIX_SOCKET, DaemonMessage, Message, MessageKind, SocketAddr, Stream, get_daemon_ip,
        get_daemon_port, read_next_message,
    },
};

/// Starts the daemon listener.
/// For each incoming [`DaemonMessage`], a new task is spawned to run the
/// corresponding handler.
#[tracing::instrument]
pub async fn start() -> Result<()> {
    // Initialize filesystem structure before starting daemon
    init_fs()?;
    info!("Daemon filesystem initialized");

    // Bind the daemon TCP listener socket
    info!("Starting TCP listening...");
    let ip = get_daemon_ip().unwrap_or(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
    let port = get_daemon_port();

    let tcp_listener = TcpListener::bind(format!("{ip}:{port}"))
        .await
        .context("When starting the daemon.")?;

    let daemon_tcp_sock = SocketAddr::from(
        tcp_listener
            .local_addr()
            .context("Failed to fetch the daemon socket from the daemon TCP listener.")?,
    );

    info!("Daemon started and listening on {}", daemon_tcp_sock);

    // Bind the daemon Unix listener socker
    info!("Starting UNIX socket listening...");
    let path = Path::new(DAEMON_UNIX_SOCKET);

    // Remove old socket file if it exists
    if path.exists() {
        remove_file(path)?;
    }

    let unix_listener = UnixListener::bind(path)?;
    info!("Daemon listening on {}", DAEMON_UNIX_SOCKET);
    let unix_addr = unix_listener
        .local_addr()
        .context("Failed to fetch local unix addr: {e:?}")?;

    // Initialising state
    let state = State::new(daemon_tcp_sock);

    // Spawn two tasks, one per listener
    let (tx, mut rx) = channel(100);

    let tcp_tx = tx.clone();
    let tcp_task = spawn(async move {
        loop {
            match tcp_listener.accept().await {
                Ok((stream, addr)) => {
                    info!("TCP connection from {}", addr);
                    if tcp_tx
                        .send((Stream::Tcp(stream), SocketAddr::from(addr)))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("TCP accept error: {}", e);
                    break;
                }
            }
        }
    });

    let unix_tx = tx.clone();
    let unix_task = spawn(async move {
        loop {
            match unix_listener.accept().await {
                Ok((stream, addr)) => {
                    info!("UNIX connection accepted on {unix_addr:?} form {addr:?}",);
                    if unix_tx
                        .send((Stream::Unix(stream), SocketAddr::from(addr)))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("UNIX accept error: {}", e);
                    break;
                }
            }
        }
    });

    // Main accept loop: handle new TCP connections
    while let Some((mut stream, addr)) = rx.recv().await {
        // Spawn a task for this connection
        let state = state.clone();
        spawn(async move {
            info!("Daemon spawned task to handle connection from {}", addr);

            loop {
                // Read next DaemonMessage from this TCP stream
                let message = match read_next_message(&mut stream, MessageKind::DaemonMessage).await
                {
                    Ok(Some(msg)) => {
                        info!("Daemon received raw DaemonMessage from {}", addr);
                        msg
                    }
                    Ok(None) => {
                        info!("Connection {} closed by peer", addr);
                        break;
                    }
                    Err(e) => {
                        warn!("Failed to read DaemonMessage from {}: {}", addr, e);
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

                match state.process_is_registered(&message.pid).await {
                    Ok(true) => info!("Process {:?} is indeed registered.", message.pid),
                    Ok(false) => {
                        if message.pid.id == 0 {
                            info!("Received a process less message.");
                        } else {
                            info!(
                                "We received a late message for process {:?}, this is ok but we ignore.",
                                message.pid
                            );
                            continue;
                        }
                    }
                    Err(e) => {
                        warn!(
                            "We failed to fetch registeration informations from the state due to {e:?}, ignoring the message, the message has to be ignored."
                        );
                        continue;
                    }
                }

                // Spawn another task for handling the specific message
                let pid = message.pid.clone();
                let ctx = MessageCtx::new(&mut stream, state.clone(), pid.clone());

                match message.inner {
                    DaemonMessage::NewProcess { makefiles, args } => {
                        info!("Handling NewProcess request from pid {:?}", pid);
                        new_process(ctx, makefiles, args).await
                    }
                    DaemonMessage::NewMakefile {
                        makefile,
                        process_datas,
                    } => {
                        info!("Handling Distribute request from pid {:?}", pid);
                        receiv_makefile(ctx, makefile, process_datas).await
                    }
                    DaemonMessage::Fetch {
                        target,
                        labeled_path,
                    } => {
                        info!(
                            "Handling Fetch request for target '{}' from pid {:?}",
                            target, pid
                        );
                        handle_fetch(ctx, target, labeled_path).await
                    }
                    DaemonMessage::StdoutLog { log } => {
                        info!("Handling new log from pid {pid:?}");
                        handle_log(ctx, log, OutputFile::Stdout).await
                    }
                    DaemonMessage::StderrLog { log } => {
                        info!("Handling new err from pid {pid:?}");
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
                    DaemonMessage::Done => handle_done(ctx).await,
                    DaemonMessage::FreshId => handle_fresh_request(ctx).await,
                }
            }
            info!("Daemon task for {} terminated", addr);
        });
    }

    try_join!(tcp_task, unix_task)?;
    Ok(())
}
