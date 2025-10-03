use crate::{
    makefile::RemoteMakefile,
    network::{Message, ProcessMessage, distribute, get_daemon_sock, utils::send_message},
    process_id::ProcessId,
};
use log::warn;
use std::{net::SocketAddr, process::Command};

pub async fn new_process(
    pid: ProcessId,
    client: SocketAddr,
    makefiles: Vec<RemoteMakefile>,
    args: Vec<String>,
) {
    if let Err(_e) = distribute(pid.clone(), makefiles).await {
        todo!("Forward the error to the caller.");
    }

    Command::new("make")
        .args(args)
        .current_dir(pid.path.clone())
        .status()
        .unwrap();

    let end_message = Message::new(ProcessMessage::End, pid, get_daemon_sock());

    if send_message(end_message, client).await.is_err() {
        warn!("Failed to send message on the caller ip.");
    }
}
