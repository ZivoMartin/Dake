use crate::network::{ProcessMessage, RemoteMakefile, distribute, utils::send_message};
use log::warn;
use std::{net::SocketAddr, path::PathBuf, process::Command};

pub async fn new_process(
    makefiles: Vec<RemoteMakefile>,
    caller_addr: SocketAddr,
    entry_makefile_dir: PathBuf,
    args: Vec<String>,
) {
    if let Err(e) = distribute(makefiles).await {
        todo!("Forward the error to the caller.")
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
