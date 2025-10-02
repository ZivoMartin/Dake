use anyhow::{Context, Result, bail};
use log::warn;
use std::{collections::HashSet, path::PathBuf, time::Duration};
use tokio::{net::TcpListener, select, sync::mpsc::channel, time::sleep};

use crate::{
    dec,
    makefile::RemoteMakefile,
    network::{
        DeamonMessage, MessageKind, get_deamon_address, messages::DistributerMessage,
        read_next_message, utils::send_message,
    },
};

pub async fn distribute(makefiles: Vec<RemoteMakefile>, path: PathBuf) -> Result<()> {
    let mut caller_sock = get_deamon_address();
    caller_sock.set_port(0);
    let listener = TcpListener::bind(caller_sock).await?;
    caller_sock = listener.local_addr()?;

    let host_amount = makefiles.len();

    if host_amount == 0 {
        return Ok(());
    }

    for mut makefile in makefiles {
        let ip = *makefile.sock();
        makefile.set_sock(caller_sock);
        send_message(DeamonMessage::Distribute(makefile, path.clone()), ip).await?;
    }

    let (sender, mut receiver) = channel(host_amount);

    let mut set = HashSet::new();

    let mut sleep_fut = Box::pin(sleep(Duration::from_secs(10)));
    let mut ack_count = 0;

    loop {
        select! {
            _ = &mut sleep_fut => bail!("The distributer timed out when waiting for acks."),
            message = receiver.recv() => {
                match message.context("Distributer: Failed to receiv message from tokio channels.")? {
                    (DistributerMessage::Ack, _) => {
                        ack_count += 1;
                        if ack_count == host_amount {
                            break
                        }
                    },
                    (DistributerMessage::Failed, sock) => {
                        bail!("Received a failed message from: {sock}")
                    }
                }

            },
            tcp_stream = listener.accept() => {
                let (mut tcp_stream, sock) = tcp_stream?;

                if !set.insert(sock) {
                    warn!("Distributer: The same address returned twice an acknowledgment: {sock}");
                    continue
                }

                let sender = sender.clone();

                tokio::spawn(async move {

                    let message = match read_next_message(&mut tcp_stream, MessageKind::DistributerMessage).await {
                        Ok(Some(message)) => message,
                        Ok(None) => {
                            warn!("Distributer: Buffer is EOF, but it should never happen when waiting for an ack.");
                            return
                        }
                        Err(e) => {
                            warn!("Distributer: When reading an ack message: {}", e.root_cause());
                            return
                        }
                    };

                    let message: DistributerMessage = match dec!(message) {
                        Ok(msg) => msg,
                        Err(e) => {
                            warn!("Distributer: Failed to decrypt a DistributerMessage: {e}");
                            return
                        }
                    };

                    if let Err(e) = sender.send((message, sock)).await {
                        warn!("Distributer: Failed to notify for the message received from {sock}: {e}");
                    }
                });

            }
        }
    }

    Ok(())
}
