use crate::{
    daemon::communication::{AckMessage, Message, MessageKind, read_next_message},
    dec,
};
use anyhow::{Context, Result, bail};
use std::{collections::HashSet, time::Duration};
use tokio::{net::TcpListener, select, sync::mpsc::channel, time::sleep};
use tracing::{error, info, warn};

pub async fn wait_acks(
    listener: &TcpListener,
    host_amount: usize,
    timeout: Option<Duration>,
) -> Result<()> {
    // Channel to collect acknowledgments
    let (sender, mut receiver) = channel(host_amount);

    // Track hosts we already received acknowledgments from
    let mut set = HashSet::new();

    // Timeout for all acknowledgments
    let mut sleep_fut = Box::pin(sleep(timeout.unwrap_or(Duration::from_secs(10))));
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

            // Accept new incoming acknowledgment connections
            tcp_stream = listener.accept() => {
                let (mut tcp_stream, sock) = tcp_stream?;

                // Prevent duplicate acks from same host
                if !set.insert(sock) {
                    warn!("The same address returned twice an acknowledgment: {sock}");
                    continue;
                }

                info!("Accepted connection from {}", sock);

                let sender = sender.clone();

                tokio::spawn(async move {
                    info!("Awaiting acknowledgment from {}", sock);

                    // Read acknowledgment message
                    let message: Vec<u8> = match read_next_message(&mut tcp_stream, MessageKind::AckMessage).await {
                        Ok(Some(message)) => message,
                        Ok(None) => {
                            warn!("Buffer EOF while waiting for ack from {}", sock);
                            return;
                        }
                        Err(e) => {
                            warn!("Failed to read ack message from {}: {}", sock, e.root_cause());
                            return;
                        }
                    };


                    let msg: Option<Message<AckMessage>> = dec!(message).ok();

                    // Forward acknowledgment to channel
                    if let Err(e) = sender.send((msg.map(|msg| msg.inner), sock)).await {
                        warn!("Failed to forward message from {} via channel: {e}", sock);
                    } else {
                        info!("Ack/Fail/Invalid message from {} forwarded to main loop", sock);
                    }
                });
            }
        }
    }
    Ok(())
}
