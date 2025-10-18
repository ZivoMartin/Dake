use anyhow::{Result, bail};
use tokio::net::TcpListener;
use tracing::warn;

use crate::{
    daemon::{
        communication::{DaemonMessage, Message, get_daemon_sock, send_message},
        operations::wait_acks,
        state::State,
    },
    process_id::ProcessId,
};

pub async fn broadcast_done(state: &State, pid: ProcessId) -> Result<()> {
    let involved_processes = match state.read_involved_hosts(&pid).await? {
        Some(involved_processes) => involved_processes,
        None => bail!("The process {pid:?} is not registered."),
    };

    let mut caller_sock = get_daemon_sock();
    caller_sock.set_port(0);

    let listener = TcpListener::bind(caller_sock).await?;
    caller_sock = listener.local_addr()?;

    let msg = Message::new(DaemonMessage::Done, pid, caller_sock);
    for sock in involved_processes.iter().copied() {
        if let Err(e) = send_message(msg.clone(), sock).await {
            warn!("Failed to contact {sock}, {e:?}");
        }
    }

    wait_acks(&listener, involved_processes.len(), None).await
}
