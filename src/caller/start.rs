use std::net::SocketAddr;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use crate::{
    caller::utils::accept_specific_connection,
    daemon::communication::{
        DaemonMessage, Message, MessageKind, ProcessMessage, read_next_message, send_message,
    },
    dec,
    makefile::RemoteMakefileSet,
    process_id::ProcessId,
};

#[tracing::instrument]
pub async fn start(
    listener: &TcpListener,
    pid: ProcessId,
    makefiles: RemoteMakefileSet,
    args: Vec<String>,
    daemon_sock: SocketAddr,
) -> Result<i32> {
    let caller_addr = listener
        .local_addr()
        .context("When requesting the caller socket address")?;

    let message = Message::new(
        DaemonMessage::NewProcess {
            makefiles: makefiles.drop_makefiles(),
            args,
        },
        pid,
        caller_addr,
    );

    info!("Sending NewProcess message to daemon at {}", daemon_sock);

    send_message(message, daemon_sock).await?;
    info!("NewProcess message delivered successfully");

    let mut tcp_stream = accept_specific_connection(&listener, daemon_sock.ip()).await?;

    info!("Caller connected to daemon stream, awaiting messages...");
    let exit_code = loop {
        // Read next message from daemon
        let msg = match read_next_message(&mut tcp_stream, MessageKind::ProcessMessage).await {
            Ok(Some(msg)) => msg,
            Ok(None) => {
                error!("Caller connection closed naturally; expected closure via End message.");
                break 1;
            }
            Err(e) => {
                warn!("Failed to receive a message from the daemon: {e}");
                continue;
            }
        };

        // Deserialize into ProcessMessage
        let msg: Message<ProcessMessage> = dec!(msg)?;
        match msg.inner {
            ProcessMessage::End { exit_code } => {
                info!("Caller received End message from daemon, build completed");
                break exit_code;
            }
            ProcessMessage::StdoutLog { log } => print!("{log}"),
            ProcessMessage::StderrLog { log } => eprint!("{log}"),
            _ => warn!("Caller should not receiv {msg:?} at this point."),
        }
    };
    Ok(exit_code)
}
