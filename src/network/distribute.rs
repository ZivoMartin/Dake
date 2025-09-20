use anyhow::{Result, bail};
use log::warn;
use std::{collections::HashSet, time::Duration};
use tokio::{net::TcpListener, pin, select, sync::mpsc::channel, time::sleep};

use crate::{
    dec,
    network::{
        DeamonMessage, MessageKind, RemoteMakefile, get_deamon_address,
        messages::DistributerMessage, read_next_message, utils::send_message,
    },
};

pub async fn distribute(makefiles: Vec<RemoteMakefile>) -> Result<()> {
    let mut caller_ip = get_deamon_address();
    caller_ip.set_port(0);
    let listener = TcpListener::bind(caller_ip).await?;
    caller_ip = listener.local_addr()?;

    let host_amount = makefiles.len();

    if host_amount == 0 {
        return Ok(());
    }

    for mut makefile in makefiles {
        let ip = *makefile.ip();
        makefile.set_ip(caller_ip);
        send_message(DeamonMessage::Distribute(makefile), ip).await?;
    }

    let (sender, mut receiver) = channel(host_amount);

    let mut set = HashSet::new();

    let mut sleep_fut = Box::pin(sleep(Duration::from_secs(10)));
    let mut ack_count = 0;

    loop {
        select! {
            _ = &mut sleep_fut => bail!("The distributer timed out when waiting for acks."),
            _ = receiver.recv() => {
                ack_count += 1;
                if ack_count == host_amount {
                    break
                }
            },
            tcp_stream = listener.accept() => {
                let (mut tcp_stream, addr) = tcp_stream?;

                if !set.insert(addr) {
                    warn!("Distributer: The same address returned twice an acknowledgment: {addr}");
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

                    match message {
                        DistributerMessage::Ack => {
                            if let Err(e) = sender.send(()).await {
                                warn!("Distributer: Failed to notify for ack from {addr}: {e}");
                            }
                        }
                    }

                });

            }
        }
    }

    Ok(())
}
