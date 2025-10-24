use std::{net::SocketAddr, time::Duration};

use tracing::warn;

use crate::daemon::{communication::MessageCtx, communication::Notif};

#[tracing::instrument]
pub async fn handle_error(
    MessageCtx { pid, state, .. }: MessageCtx,
    guilty_node: SocketAddr,
    exit_code: i32,
) {
    let notif = Notif::Error {
        guilty_node,
        exit_code,
    };
    match state.notifier_hub().lock().await.arc_send(notif, &pid) {
        Ok(w) => {
            if let Err(e) = w.wait(Some(Duration::from_secs(1))).await {
                warn!("Failed to wait for notif publication: {e:?}")
            }
        }
        Err(e) => warn!("The channel for {pid:?} was not initialised: {e:?}"),
    }
}
