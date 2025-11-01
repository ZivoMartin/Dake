use tracing::{error, info, warn};

use crate::{
    daemon::MessageCtx,
    network::{Message, ProcessMessage, write_message},
    process_id::{ProcessId, ProjectId},
};

#[tracing::instrument(skip(state, stream), fields(project_id = %pid.project_id))]
pub async fn handle_fresh_request<'a>(
    MessageCtx {
        pid,
        stream,
        mut state,
    }: MessageCtx<'a>,
) {
    info!("Starting to handle fresh ID request");

    let daemon_id = match state.config() {
        Ok(config) => {
            let id = config.id();
            info!("Just fetched the daemon id: {id}");
            id
        }
        Err(e) => {
            warn!("Failed to fetch config: {e}");
            return;
        }
    };
    let project_id = ProjectId::new(daemon_id, pid.path().clone());

    info!("Fetching fresh ID for project: {:?}", project_id);
    let id = match state.get_fresh_id(project_id.clone()).await {
        Ok(id) => {
            info!(%id, "Successfully fetched fresh ID");
            id
        }
        Err(e) => {
            error!(error = ?e, "Failed to fetch a fresh ID from the state");
            return;
        }
    };

    info!(%id, "Constructing new ProcessId");
    let pid = ProcessId::new(id, project_id.daemon_id, project_id.path.clone());
    state.register_process(pid.clone()).await;

    info!(?pid, "Created new ProcessId, preparing to send response");
    let msg = Message::new(ProcessMessage::FreshId, pid);

    info!("Sending response message");
    if let Err(e) = write_message(stream, msg).await {
        warn!(error = ?e, "Failed to send FreshId response; the state was updated anyway");
    } else {
        info!("Successfully sent FreshId response");
    }

    info!("Finished handling fresh ID request");
}
