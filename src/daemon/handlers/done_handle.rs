use std::time::Duration;

use notifier_hub::notifier::ChannelState;
use tokio::{select, time::sleep};
use tracing::{info, warn};

use crate::daemon::communication::{AckMessage, Message, MessageCtx, Notif, send_message};

#[tracing::instrument]
pub async fn handle_done(MessageCtx { pid, state, client }: MessageCtx) {
    match state.remove_process(&pid).await {
        Ok(Some(data)) => {
            info!("Successfuly removed {pid:?} from the processes database, got {data:?}.")
        }
        Ok(None) => warn!("The process database do not contain {pid:?}"),
        Err(e) => warn!("Failed to lock processes database: {e:?}"),
    }
    let waiter = {
        let hub = state.notifier_hub();
        let sleep_fut = Box::pin(sleep(Duration::from_secs(5)));
        select! {
            _ = sleep_fut => {
                warn!("Failed to lock notifier_hub");
                None
            }
            notifier_hub = hub.lock() => {
                match notifier_hub.channel_state(&pid) {
                    ChannelState::Uninitialised => None,
                    ChannelState::Running => {
                        match notifier_hub.arc_send(Notif::Done, &pid) {
                            Ok(w) => Some(w),
                            Err(e) => {
                                warn!("Failed to publish the done notification over notifier_hub {e:?}");
                                None
                            }
                        }
                    }
                    ChannelState::Over => {
                        warn!("The channel {pid:?} is not already over in notifier_hub.");
                        None
                    },
                }
            }
        }
    };
    if let Some(w) = waiter {
        if let Err(e) = w.wait(Some(Duration::from_secs(3))).await {
            warn!("Failed to wait for done notif publication: {e:?}")
        }
    }

    if let Err(e) = send_message(
        Message::new(AckMessage::Ok, pid.clone(), state.daemon_sock),
        client,
    )
    .await
    {
        warn!("Failed to send Ack to main daemon {client} for pid {pid:?}: {e}",);
    } else {
        info!("Ack successfully sent to {}", client);
    }
}
