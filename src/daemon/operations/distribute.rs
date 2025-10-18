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

use anyhow::Result;

use tokio::net::TcpListener;
use tracing::info;

use crate::{
    daemon::{
        communication::{DaemonMessage, Message, get_daemon_sock, send_message},
        operations::wait_acks::wait_acks,
        process_datas::ProcessDatas,
    },
    makefile::RemoteMakefile,
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

    wait_acks(&listener, host_amount, None).await
}
