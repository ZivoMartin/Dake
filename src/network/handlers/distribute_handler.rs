//! # Distribute Module
//!
//! This module defines the logic for distributing remote makefiles to different
//! hosts in the Dake distributed build system.  
//!
//! The distribute workflow is as follows:
//! 1. Bind a temporary listener socket for acknowledgments.
//! 2. Send each host a `DaemonMessage::Distribute` containing its `RemoteMakefile`.
//! . Wait for acknowledgments (`DistributerMessage::Ack`) or failures
//!    (`DistributerMessage::Failed`) from all hosts.
//! 4. Return success only if all hosts acknowledged within the timeout.
//!
//! If acknowledgments are missing after a timeout, or if any host sends a
//! `Failed` message, the distributor aborts with an error.

use anyhow::{Context, Result, bail, ensure};
use std::{collections::HashSet, time::Duration};
use tokio::{net::TcpListener, select, sync::mpsc::channel, time::sleep};
use tracing::{error, info, warn};

use crate::{
    dec,
    makefile::RemoteMakefile,
    network::{
        DaemonMessage, Message, MessageKind, get_daemon_sock, messages::DistributerMessage,
        process_datas::ProcessDatas, read_next_message, utils::send_message,
    },
    process_id::ProcessId,
};

/// Distributes a list of makefiles to remote hosts and waits for acknowledgments.
/// Returns an error if:
/// - Binding or accepting sockets fails.
/// - Sending messages to remote hosts fails.
/// - Not all acknowledgments are received within the timeout.
pub async fn distribute(
    pid: ProcessId,
    makefiles: Vec<RemoteMakefile>,
    process_datas: ProcessDatas,
) -> Result<()> {
    ensure!(
        process_datas.is_remote(),
        "The process datas given to distribute should not be local."
    );

    // Prepare a temporary listener for acknowledgments
    let mut caller_sock = get_daemon_sock();
    caller_sock.set_port(0);

    info!("Distributer: Binding acknowledgment listener on ephemeral port");
    let listener = TcpListener::bind(caller_sock).await?;
    caller_sock = listener.local_addr()?;
    info!(
        "Distributer: Acknowledgment listener bound at {}",
        caller_sock
    );

    let host_amount = makefiles.len();
    info!(
        "Distributer: Preparing to distribute {} makefiles",
        host_amount
    );

    // Nothing to distribute
    if host_amount == 0 {
        info!("Distributer: No makefiles to distribute, returning immediately");
        return Ok(());
    }

    // Send makefiles to each host
    for makefile in makefiles {
        let sock = *makefile.sock();
        let message = Message::new(
            DaemonMessage::NewMakefile {
                makefile,
                process_datas: process_datas.clone(),
            },
            pid.clone(),
            caller_sock,
        );

        info!("Distributer: Sending makefile to host {}", sock);
        send_message(message, sock).await?;
        info!("Distributer: Makefile sent successfully to {}", sock);
    }

    // Channel to collect acknowledgments
    let (sender, mut receiver) = channel(host_amount);

    // Track hosts we already received acknowledgments from
    let mut set = HashSet::new();

    // Timeout for all acknowledgments
    let mut sleep_fut = Box::pin(sleep(Duration::from_secs(10)));
    let mut ack_count = 0;

    loop {
        select! {
            // Timeout branch
            _ = &mut sleep_fut => {
                error!("Distributer: Timed out after 10s while waiting for acks");
                bail!("The distributer timed out when waiting for acks.");
            },

            // Process messages received via channel
            message = receiver.recv() => {
                match message.context("Distributer: Failed to receive message from tokio channel")? {
                    (DistributerMessage::Ack, sock) => {
                        ack_count += 1;
                        info!("Distributer: Received Ack from {} (ack_count={}/{})", sock, ack_count, host_amount);

                        if ack_count == host_amount {
                            info!("Distributer: All {} acknowledgments received successfully", host_amount);
                            break;
                        }
                    },
                    (DistributerMessage::Failed, sock) => {
                        error!("Distributer: Received failure message from {}", sock);
                        bail!("Received a failed message from: {sock}");
                    }
                }
            },

            // Accept new incoming acknowledgment connections
            tcp_stream = listener.accept() => {
                let (mut tcp_stream, sock) = tcp_stream?;

                // Prevent duplicate acks from same host
                if !set.insert(sock) {
                    warn!("Distributer: The same address returned twice an acknowledgment: {sock}");
                    continue;
                }

                info!("Distributer: Accepted connection from {}", sock);

                let sender = sender.clone();

                tokio::spawn(async move {
                    info!("Distributer: Awaiting acknowledgment from {}", sock);

                    // Read acknowledgment message
                    let message = match read_next_message(&mut tcp_stream, MessageKind::DistributerMessage).await {
                        Ok(Some(message)) => message,
                        Ok(None) => {
                            warn!("Distributer: Buffer EOF while waiting for ack from {}", sock);
                            return;
                        }
                        Err(e) => {
                            warn!("Distributer: Failed to read ack message from {}: {}", sock, e.root_cause());
                            return;
                        }
                    };

                    // Deserialize acknowledgment
                    let message: Message<DistributerMessage> = match dec!(message) {
                        Ok(msg) => msg,
                        Err(e) => {
                            warn!("Distributer: Failed to decrypt DistributerMessage from {}: {e}", sock);
                            return;
                        }
                    };

                    // Forward acknowledgment to channel
                    if let Err(e) = sender.send((message.inner, sock)).await {
                        warn!("Distributer: Failed to forward message from {} via channel: {e}", sock);
                    } else {
                        info!("Distributer: Ack/Fail message from {} forwarded to main loop", sock);
                    }
                });
            }
        }
    }

    Ok(())
}
