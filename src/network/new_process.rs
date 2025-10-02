use crate::{
    makefile::RemoteMakefile,
    network::{ProcessMessage, distribute, utils::send_message},
};
use log::warn;
use std::{net::SocketAddr, path::PathBuf, process::Command};

pub async fn new_process(
    makefiles: Vec<RemoteMakefile>,
    caller_addr: SocketAddr,

    entry_makefile_dir: PathBuf,
    args: Vec<String>,
) {
    if let Err(_e) = distribute(makefiles, entry_makefile_dir.clone()).await {
        todo!("Forward the error to the caller.");
    }

    Command::new("make")
        .args(args)
        .current_dir(entry_makefile_dir)
        .status()
        .unwrap();
    if send_message(ProcessMessage::End, caller_addr)
        .await
        .is_err()
    {
        warn!("Failed to send message on the caller ip.");
    }
}
