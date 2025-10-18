use tracing::{error, warn};

use crate::{
    daemon::communication::{Message, MessageCtx, ProcessMessage, get_daemon_sock, send_message},
    process_id::ProcessId,
};

pub async fn handle_fresh_request(MessageCtx { pid, client, state }: MessageCtx) {
    let project_id = pid.project_id;
    let id = match state.get_fresh_id(project_id.clone()).await {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to fetch a fresh id from the state: {e:?}");
            return;
        }
    };
    let pid = ProcessId::new(id, project_id.sock, project_id.path);
    let msg = Message::new(ProcessMessage::FreshId, pid, get_daemon_sock());
    if let Err(e) = send_message(msg, client).await {
        warn!(
            "Failed to answer to the fresh id request, the database have been updated anyway. {e:?}."
        )
    }
}
