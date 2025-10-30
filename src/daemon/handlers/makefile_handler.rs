use tracing::{error, info, warn};

use crate::{
    daemon::{MessageCtx, fs::push_makefile, process_datas::ProcessDatas},
    makefile::RemoteMakefile,
    network::{AckMessage, Message, write_message},
};

/// Receives a remote makefile, writes it to disk, and replies with an acknowledgment.
#[tracing::instrument(skip(stream, state, makefile))]
pub async fn receiv_makefile<'a>(
    MessageCtx { pid, stream, state }: MessageCtx<'a>,
    makefile: RemoteMakefile,
    process_datas: ProcessDatas,
) {
    info!("Handling incoming makefile: {}", makefile.to_string());

    // Closure to simplify message creation with same pid and client
    let message = |inner| Message::new(inner, pid.clone());

    // Registering the new makefile in the shared database
    state
        .set_process_datas(process_datas.pid.clone(), process_datas)
        .await;

    // Attempt to persist makefile
    match push_makefile(&makefile, &pid) {
        Ok(_) => {
            info!("Successfully persisted makefile for pid {pid:?}, sending Ack");
            if let Err(e) = write_message(stream, message(AckMessage::Ok)).await {
                warn!("Failed to send Ack to distributor for pid {:?}: {e}", pid);
            } else {
                info!("Ack successfully sent.");
            }
        }
        Err(e) => {
            error!("Failed to persist makefile for pid {:?}: {e}", pid);
            if let Err(e) = write_message(stream, message(AckMessage::Failure)).await {
                warn!(
                    "Failed to send Fail message to distributor for pid {:?}: {e}",
                    pid
                );
            } else {
                info!("Failure message sent to distributor for pid {:?}", pid);
            }
        }
    }
}
