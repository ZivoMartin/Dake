use anyhow::{Result, bail};
use tracing::warn;

use crate::{
    dec,
    network::{
        DaemonMessage, Message, MessageKind, ProcessMessage, Stream, read_next_message,
        write_message,
    },
    process_id::{ProcessId, ProjectId},
};

pub async fn fetch_fresh_id(stream: &mut Stream, pid: ProjectId) -> Result<ProcessId> {
    let default_pid = ProcessId::new_default(pid);
    let inner = DaemonMessage::FreshId;
    let msg = Message::new(inner, default_pid);

    write_message(stream, msg).await?;

    let pid = loop {
        // Read next message from daemon
        let msg = match read_next_message(stream, MessageKind::ProcessMessage).await {
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
