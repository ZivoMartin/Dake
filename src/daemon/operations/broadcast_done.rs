use anyhow::{Result, bail};

use crate::{
    daemon::{State, operations::wait_acks},
    network::{DaemonMessage, Message, broadcast_message},
    process_id::ProcessId,
};

pub async fn broadcast_done(state: &State, pid: ProcessId) -> Result<()> {
    let involved_processes = match state.read_involved_hosts(&pid).await? {
        Some(involved_processes) => involved_processes,
        None => bail!("The process {pid:?} is not registered."),
    };

    let message = Message::new(DaemonMessage::Done, pid);

    let mut streams = broadcast_message(involved_processes, message).await?;
    let streams = streams.iter_mut().collect();
    wait_acks(streams, None).await
}
