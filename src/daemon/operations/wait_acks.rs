use crate::{
    dec,
    network::{AckMessage, Message, MessageKind, Stream, read_next_message},
};
use anyhow::{Result, bail};
use futures::future::join_all;
use std::time::Duration;
use tokio::{select, sync::mpsc::channel, time::sleep};
use tracing::{error, info, warn};

#[tracing::instrument(skip(streams, timeout))]
pub async fn wait_acks<'a>(streams: Vec<&'a mut Stream>, timeout: Option<Duration>) -> Result<()> {
    let host_amount = streams.len();

    info!("Waiting for {host_amount} acks");

    // Channel to collect acknowledgments
    let (sender, mut receiver) = channel(streams.len());

    let tasks = streams
        .into_iter()
        .map(|stream| {
            let sender = sender.clone();
            async move {
                let sock = match stream.peer_addr() {
                    Ok(sock) => sock,
                    Err(e) => {
                        error!("Failed to fetch peer address on stream: {e}");
                        return;
                    }
                };

                info!("Awaiting an acknowledgment from {}", sock);

                // Read acknowledgment message
                let message: Vec<u8> =
                    match read_next_message(stream, MessageKind::AckMessage).await {
                        Ok(Some(message)) => message,
                        Ok(None) => {
                            warn!("Buffer EOF while waiting for ack from {}", sock);
                            return;
                        }
                        Err(e) => {
                            warn!(
                                "Failed to read ack message from {}: {}",
                                sock,
                                e.root_cause()
                            );
                            return;
                        }
                    };
                info!("Received a new ack from {sock}");
                let msg: Option<Message<AckMessage>> = dec!(message)
                    .inspect_err(|e| warn!("Failed to decode the ack of {sock}: {e}"))
                    .inspect(|_| info!("Successfully decoded the ack of {sock}"))
                    .ok();

                // Forward acknowledgment to channel
                if let Err(e) = sender.send((msg.map(|msg| msg.inner), sock.clone())).await {
                    error!("Failed to forward message from {} via channel: {e}", sock);
                } else {
                    info!(
                        "Ack/Fail/Invalid message from {} forwarded to main loop",
                        sock
                    );
                }
            }
        })
        .collect::<Vec<_>>();

    // Timeout for all acknowledgments
    let all_tasks = join_all(tasks);
    let mut sleep_fut = Box::pin(sleep(timeout.unwrap_or(Duration::from_secs(30))));

    select! {
        // Timeout branch
        _ = &mut sleep_fut => {
            error!("Timed out after 10s while waiting for acks");
            bail!("Timed out when waiting for acks.");
        },

        _ = all_tasks => {
            let mut ack_count = 0;

            // Process messages received via channel
            while let Ok(message) = receiver.try_recv() {
                match message {
                    (Some(AckMessage::Ok), sock) => {
                        ack_count += 1;
                        info!("Received Ack from {} (ack_count={}/{})", sock, ack_count, host_amount);
                    },
                    (Some(AckMessage::Failure), sock) => {
                        bail!("Received a failed message from: {sock}");
                    }
                    (None, sock) => {
                        bail!("Received an invalid message from {sock}");
                    }
                }
            }
            if ack_count == host_amount {
                info!("All {} acknowledgments received successfully", host_amount);
            } else {
                bail!("Failed to receiv all the acknowledgments.")
            }

        },
    }

    Ok(())
}
