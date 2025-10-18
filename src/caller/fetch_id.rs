use anyhow::{Context, Result, bail};
use tokio::net::TcpListener;
use tracing::warn;

use crate::{
    caller::utils::accept_specific_connection,
    daemon::communication::{
        DaemonMessage, Message, MessageKind, ProcessMessage, contact_daemon_or_start_it,
        get_daemon_sock, read_next_message,
    },
    dec,
    process_id::{ProcessId, ProjectId},
};

pub async fn fetch_fresh_id(listener: &TcpListener, pid: ProjectId) -> Result<ProcessId> {
    let daemon_sock = get_daemon_sock();
    let caller_addr = listener
        .local_addr()
        .context("When requesting the caller socket address")?;

    let default_pid = ProcessId::new_default(pid);
    let inner = DaemonMessage::FreshId;
    let msg = Message::new(inner, default_pid, caller_addr);

    contact_daemon_or_start_it(msg).await?;

    let mut tcp_stream = accept_specific_connection(&listener, daemon_sock.ip()).await?;

    let pid = loop {
        // Read next message from daemon
        let msg = match read_next_message(&mut tcp_stream, MessageKind::ProcessMessage).await {
            Ok(Some(msg)) => msg,
            Ok(None) => {
                bail!("Caller connection closed naturally; Was waiting for the fresh process id.");
            }
            Err(e) => {
                warn!("Failed to receive a message from the daemon: {e}");
                continue;
            }
        };

        let msg: Message<ProcessMessage> = dec!(msg)?;
        match msg.inner {
            ProcessMessage::FreshId => break msg.pid,
            _ => warn!("Was waiting for a fresh pid, received {msg:?}"),
        }
    };

    Ok(pid)
}
