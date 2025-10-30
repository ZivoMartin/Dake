use anyhow::{Context, Result};
use futures::future::join_all;
use tokio::spawn;
use tracing::info;

use crate::network::{Message, MessageTrait, SocketAddr, Stream, connect, write_message};

pub async fn broadcast_message<M>(
    network: Vec<SocketAddr>,
    message: Message<M>,
) -> Result<Vec<Stream>>
where
    M: MessageTrait,
{
    let n = network.len();
    broadcast_messages(network, vec![message; n]).await
}

#[tracing::instrument(skip(messages, network))]
pub async fn broadcast_messages<M>(
    network: Vec<SocketAddr>,
    messages: Vec<Message<M>>,
) -> Result<Vec<Stream>>
where
    M: MessageTrait,
{
    info!("Broadcasting {} messages to {network:?}", messages.len());

    info!("Connecting to each hosts..");
    let connect_tasks = network
        .iter()
        .cloned()
        .map(|sock| {
            spawn(async move {
                connect(sock.clone())
                    .await
                    .context(format!("Failed to connect to the host {sock}"))
            })
        })
        .collect::<Vec<_>>();

    // Wait for all tasks to finish
    let connect_results = join_all(connect_tasks).await;

    // Handle both spawn errors and connection errors
    let mut streams: Vec<Stream> = connect_results
        .into_iter()
        .zip(network)
        .map(|(join_res, sock)| {
            join_res.context(format!("Failed to connect to the host {sock}"))?
        })
        .collect::<Result<Vec<_>, _>>()?;

    info!("Connected successfully.");

    for (stream, message) in streams.iter_mut().zip(messages.into_iter()) {
        info!("Sending broadcast message to host {}", stream.peer_addr()?);
        write_message(stream, message).await?;
    }

    Ok(streams)
}
