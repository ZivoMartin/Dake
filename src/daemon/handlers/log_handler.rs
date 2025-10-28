use std::time::Duration;

use tracing::warn;

use crate::daemon::{MessageCtx, Notif};

#[derive(Debug)]
pub enum OutputFile {
    Stdout,
    Stderr,
}

#[tracing::instrument]
pub async fn handle_log<'a>(
    MessageCtx { pid, state, .. }: MessageCtx<'a>,
    log: String,
    output: OutputFile,
) {
    let notif = Notif::Log { log, output };
    match state.notifier_hub().lock().await.arc_send(notif, &pid) {
        Ok(w) => {
            if let Err(e) = w.wait(Some(Duration::from_secs(1))).await {
                warn!("Failed to wait for notif publication: {e:?}")
            }
        }
        Err(e) => warn!("The channel for {pid:?} was not initialised: {e:?}"),
    }
}
