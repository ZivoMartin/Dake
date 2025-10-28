use crate::{
    dec,
    network::{AckMessage, Message, MessageKind, Stream, read_next_message},
};
use anyhow::{Context, Result, bail};
use futures::future::join_all;
use std::time::Duration;
use tokio::{select, sync::mpsc::channel, time::sleep};
use tracing::{error, info, warn};

pub async fn wait_acks<'a>(streams: Vec<&'a mut Stream>, timeout: Option<Duration>) -> Result<()> {
    let host_amount = streams.len();

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

                info!("Awaiting acknowledgment from {}", sock);

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

                let msg: Option<Message<AckMessage>> = dec!(message).ok();

                // Forward acknowledgment to channel
                if let Err(e) = sender.send((msg.map(|msg| msg.inner), sock.clone())).await {
                    warn!("Failed to forward message from {} via channel: {e}", sock);
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
    let mut sleep_fut = Box::pin(sleep(timeout.unwrap_or(Duration::from_secs(30))));
    let mut ack_count = 0;

    loop {
        select! {
            // Timeout branch
            _ = &mut sleep_fut => {
                error!("Timed out after 10s while waiting for acks");
                bail!("Timed out when waiting for acks.");
            },

            // Process messages received via channel
            message = receiver.recv() => {
                match message.context("Failed to receive message from tokio channel")? {
                    (Some(AckMessage::Ok), sock) => {
                        ack_count += 1;
                        info!("Received Ack from {} (ack_count={}/{})", sock, ack_count, host_amount);

                        if ack_count == host_amount {
                            info!("All {} acknowledgments received successfully", host_amount);
                            break
                        }
                    },
                    (Some(AckMessage::Failure), sock) => {
                        bail!("Received a failed message from: {sock}");
                    }
                    (None, sock) => {
                        bail!("Received an invalid message from {sock}");
                    }
                }
            },
        }
    }

    join_all(tasks).await;
    Ok(())
}
