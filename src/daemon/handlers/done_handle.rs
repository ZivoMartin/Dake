use notifier_hub::notifier::ChannelState;
use tracing::{info, warn};

use crate::{
    constants::DONE_NOTIFICATION_TIMEOUT,
    daemon::{MessageCtx, Notif},
    lock,
    network::{AckMessage, Message, write_message},
};

#[tracing::instrument]
pub async fn handle_done<'a>(MessageCtx { pid, state, stream }: MessageCtx<'a>) {
    match state.remove_process(&pid).await {
        Ok(Some(data)) => {
            info!("Successfuly removed {pid:?} from the processes database, got {data:?}.")
        }
        Ok(None) => warn!("The process database do not contain {pid:?}"),
        Err(e) => warn!("Failed to lock processes database: {e:?}"),
    }

    let waiter = {
        let hub = state.notifier_hub();
        match lock!(hub).await {
            Ok(notifier_hub) => match notifier_hub.channel_state(&pid) {
                ChannelState::Uninitialised => None,
                ChannelState::Running => match notifier_hub.arc_send(Notif::Done, &pid) {
                    Ok(w) => Some(w),
                    Err(e) => {
                        warn!("Failed to publish the done notification over notifier_hub {e:?}");
                        None
                    }
                },
                ChannelState::Over => {
                    warn!("The channel {pid:?} is not already over in notifier_hub.");
                    None
                }
            },
            Err(_) => {
                warn!("Failed to lock notifier_hub");
                None
            }
        }
    };
    if let Some(w) = waiter {
        if let Err(e) = w.wait(Some(DONE_NOTIFICATION_TIMEOUT)).await {
            warn!("Failed to wait for done notif publication: {e:?}")
        }
    }

    if let Err(e) = write_message(stream, Message::new(AckMessage::Ok, pid.clone())).await {
        warn!("Failed to send Ack to main daemon for pid {pid:?}: {e}",);
    } else {
        info!("Ack successfully sent to main daemon");
    }
}
