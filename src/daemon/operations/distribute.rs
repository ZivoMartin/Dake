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

use tracing::info;

use crate::{
    daemon::{operations::wait_acks::wait_acks, process_datas::ProcessDatas, state::State},
    makefile::RemoteMakefile,
    network::{DaemonMessage, Message, SocketAddr, broadcast_messages},
    process_id::ProcessId,
};

/// Distributes a list of makefiles to remote hosts and waits for acknowledgments.
/// Returns an error if:
/// - Binding or accepting sockets fails.
/// - Sending messages to remote hosts fails.
/// - Not all acknowledgments are received within the timeout.
#[tracing::instrument]
pub async fn distribute(
    state: &State,
    pid: ProcessId,
    makefiles: Vec<RemoteMakefile>,
    process_datas: ProcessDatas,
) -> Result<()> {
    let host_amount = makefiles.len();
    info!("Preparing to distribute {} makefiles", host_amount);

    // Nothing to distribute
    if host_amount == 0 {
        info!("Distributer: No makefiles to distribute, returning immediately");
        return Ok(());
    }

    let socks = makefiles
        .iter()
        .map(|makefile| SocketAddr::from(makefile.sock().clone()))
        .collect::<Vec<_>>();

    let messages = makefiles
        .iter()
        .cloned()
        .map(|makefile| {
            Message::new(
                DaemonMessage::NewMakefile {
                    makefile,
                    process_datas: process_datas.clone(),
                },
                pid.clone(),
            )
        })
        .collect::<Vec<_>>();

    let mut streams = broadcast_messages(socks, messages).await?;
    let streams = streams.iter_mut().collect();
    wait_acks(streams, None).await
}
