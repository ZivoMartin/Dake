use std::time::Duration;

use tracing::warn;

use crate::{
    daemon::{MessageCtx, Notif},
    lock,
    network::SocketAddr,
};

#[tracing::instrument]
pub async fn handle_error<'a>(
    MessageCtx { pid, state, .. }: MessageCtx<'a>,
    guilty_node: SocketAddr,
    exit_code: i32,
) {
    let notif = Notif::Error {
        guilty_node,
        exit_code,
    };

    let w = {
        let notifier_hub = state.notifier_hub().clone();
        let notifier_hub = match lock!(notifier_hub).await {
            Ok(hub) => hub,
            Err(e) => {
                warn!("Failed to lock notifier_hub: {e}");
                return;
            }
        };

        match notifier_hub.arc_send(notif, &pid) {
            Ok(w) => w,
            Err(e) => {
                warn!("The channel for {pid:?} was not initialised: {e:?}");
                return;
            }
        }
    };

    if let Err(e) = w.wait(Some(Duration::from_secs(1))).await {
        warn!("Failed to wait for notif publication: {e:?}")
    }
}
