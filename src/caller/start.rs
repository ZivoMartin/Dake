use anyhow::Result;
use tracing::{error, info, warn};

use crate::{
    dec,
    makefile::RemoteMakefileSet,
    network::{DaemonMessage, Message, MessageKind, ProcessMessage, read_next_message},
    network::{Stream, write_message},
    process_id::ProcessId,
};

#[tracing::instrument]
pub async fn start(
    stream: &mut Stream,
    pid: ProcessId,
    makefiles: RemoteMakefileSet,
    args: Vec<String>,
) -> Result<i32> {
    let message = Message::new(
        DaemonMessage::NewProcess {
            makefiles: makefiles.drop_makefiles(),
            args,
        },
        pid,
    );

    info!("Sending NewProcess message to daemon.");

    write_message(stream, message).await?;
    info!("NewProcess message delivered successfully");

    info!("Caller connected to daemon stream, awaiting messages...");
    let exit_code = loop {
        // Read next message from daemon
        let msg = match read_next_message(stream, MessageKind::ProcessMessage).await {
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
