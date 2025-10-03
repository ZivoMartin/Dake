use std::net::SocketAddr;

use log::warn;

use crate::{
    makefile::RemoteMakefile,
    network::{Message, fs::push_makefile, messages::DistributerMessage, utils::send_message},
    process_id::ProcessId,
};

pub async fn receiv_makefile(pid: ProcessId, client: SocketAddr, makefile: RemoteMakefile) {
    let message = |inner| Message::new(inner, pid.clone(), client);
    match push_makefile(&makefile, &pid) {
        Ok(_) => {
            if let Err(e) = send_message(message(DistributerMessage::Ack), client).await {
                warn!("Failed to return the ack to the ditributer: {e}");
            }
        }
        Err(e) => {
            warn!("Failed to push the makefile: {e}");
            if let Err(e) = send_message(message(DistributerMessage::Failed), client).await {
                warn!("Failed to send fail message to the ditributer: {e}");
            }
        }
    }
}
