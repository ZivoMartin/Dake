use std::path::PathBuf;

use log::warn;

use crate::{
    makefile::RemoteMakefile,
    network::{fs::push_makefile, messages::DistributerMessage, utils::send_message},
};

pub async fn receiv_makefile(makefile: RemoteMakefile, path: PathBuf) {
    let sock = *makefile.sock();
    match push_makefile(&makefile, &path) {
        Ok(_) => {
            if let Err(e) = send_message(DistributerMessage::Ack, sock).await {
                warn!("Failed to return the ack to the ditributer: {e}");
            }
        }
        Err(e) => {
            warn!("Failed to push the makefile: {e}");
            if let Err(e) = send_message(DistributerMessage::Failed, sock).await {
                warn!("Failed to send fail message to the ditributer: {e}");
            }
        }
    }
}
