use std::net::SocketAddr;

use tracing::warn;

use crate::network::{Message, ProcessMessage, message_ctx::MessageCtx, send_message};

pub async fn handle_error(
    MessageCtx { state, pid, client }: MessageCtx,
    guilty_node: SocketAddr,
    exit_code: u32,
) {
    // Aborting process on all hosts

    // Fetching the client socket from the pid
    let socket_client = match state.read_client_or_warn(&pid).await {
        Some(socket_client) => socket_client,
        None => return,
    };

    // Sending the dake error message
    let inner = ProcessMessage::StderrLog {
        log: format!("Dake** Host {guilty_node} encountered a fatal error."),
    };
    let msg = Message::new(inner, pid.clone(), client);

    if let Err(e) = send_message(msg, socket_client).await {
        warn!("Logger: Failed to send fatal error message to the {client} because of {e}");
    }

    // Sending the end message itself
    let inner = ProcessMessage::End { exit_code };
    let msg = Message::new(inner, pid, client);

    if let Err(e) = send_message(msg, socket_client).await {
        warn!("Logger: Failed to send end message to the {client} because of {e}");
    }
}
