//! # Makefile Receiver
//!
//! This module handles receiving remote makefiles sent during distribution.  
//!
//! Responsibilities:
//! - Persist the received makefile to the filesystem via [`push_makefile`].
//! - Acknowledge the distributor with a [`DistributerMessage::Ack`] on success.
//! - Notify failure with a [`DistributerMessage::Failed`] if writing fails.

use tracing::{error, info, warn};

use crate::{
    makefile::RemoteMakefile,
    network::{
        Message, fs::push_makefile, message_ctx::MessageCtx, messages::DistributerMessage,
        process_datas::ProcessDatas, utils::send_message,
    },
};

/// Receives a remote makefile, writes it to disk, and replies with an acknowledgment.
pub async fn receiv_makefile(
    MessageCtx { pid, client, state }: MessageCtx,
    makefile: RemoteMakefile,
    process_datas: ProcessDatas,
) {
    info!(
        "Receiver: Handling incoming makefile for pid {:?} from client {}",
        pid, client
    );

    // Closure to simplify message creation with same pid and client
    let message = |inner| Message::new(inner, pid.clone(), client);

    // Registering the new makefile in the shared database
    state.register_process(pid.clone(), process_datas);

    // Attempt to persist makefile
    match push_makefile(&makefile, &pid) {
        Ok(_) => {
            info!(
                "Receiver: Successfully persisted makefile for pid {:?}, sending Ack to {}",
                pid, client
            );
            if let Err(e) = send_message(message(DistributerMessage::Ack), client).await {
                warn!(
                    "Receiver: Failed to send Ack to distributor {} for pid {:?}: {e}",
                    client, pid
                );
            } else {
                info!("Receiver: Ack successfully sent to {}", client);
            }
        }
        Err(e) => {
            error!(
                "Receiver: Failed to persist makefile for pid {:?}: {e}",
                pid
            );
            if let Err(e) = send_message(message(DistributerMessage::Failed), client).await {
                warn!(
                    "Receiver: Failed to send Fail message to distributor {} for pid {:?}: {e}",
                    client, pid
                );
            } else {
                info!(
                    "Receiver: Failure message sent to distributor {} for pid {:?}",
                    client, pid
                );
            }
        }
    }
}
