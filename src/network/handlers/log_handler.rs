use tracing::warn;

use crate::network::{Message, ProcessMessage, message_ctx::MessageCtx, send_message};

pub enum OutputFile {
    Stdout,
    Stderr,
}

pub async fn handle_log(
    MessageCtx { state, pid, client }: MessageCtx,
    log: String,
    output: OutputFile,
) {
    let socket_client = match state.read_client_or_warn(&pid).await {
        Some(socket_client) => socket_client,
        None => return,
    };

    let inner = match output {
        OutputFile::Stdout => ProcessMessage::StdoutLog { log },
        OutputFile::Stderr => ProcessMessage::StderrLog { log },
    };

    let msg = Message::new(inner, pid, client);

    if let Err(e) = send_message(msg, socket_client).await {
        warn!("Logger: Failed to send a packet to the {client} because of {e}");
    }
}
