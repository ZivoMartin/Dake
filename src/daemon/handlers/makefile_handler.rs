use tracing::{error, info, warn};

use crate::{
    daemon::{
        communication::{AckMessage, Message, MessageCtx, send_message},
        fs::push_makefile,
        process_datas::ProcessDatas,
    },
    makefile::RemoteMakefile,
};

/// Receives a remote makefile, writes it to disk, and replies with an acknowledgment.
#[tracing::instrument]
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
    state.register_process(pid.clone(), process_datas).await;

    // Attempt to persist makefile
    match push_makefile(&makefile, &pid) {
        Ok(_) => {
            info!(
                "Receiver: Successfully persisted makefile for pid {:?}, sending Ack to {}",
                pid, client
            );
            if let Err(e) = send_message(message(AckMessage::Ok), client).await {
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
            if let Err(e) = send_message(message(AckMessage::Failure), client).await {
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
