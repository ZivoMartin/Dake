use std::{net::SocketAddr, process::Command, time::Duration};

use anyhow::{Context, Result, bail};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ErrorKind},
    net::TcpStream,
    spawn,
    time::timeout,
};

use crate::{
    dec, enc,
    network::{
        DEFAULT_SOCK, MessageKind,
        messages::{Message, MessageHeader, MessageTrait},
    },
};

pub async fn send_message<M: MessageTrait>(msg: Message<M>, sock: SocketAddr) -> Result<()> {
    let mut stream = TcpStream::connect(sock)
        .await
        .context("When connecting on the stream.")?;
    let enc_msg = MessageHeader::wrap(enc!(msg), msg.get_kind());
    stream
        .write_all(&enc_msg)
        .await
        .context("When writing on the stream")?;
    Ok(())
}

pub fn get_daemon_sock() -> SocketAddr {
    DEFAULT_SOCK
}

pub async fn contact_daemon<M: MessageTrait>(msg: Message<M>) -> Result<()> {
    let sock = get_daemon_sock();
    send_message(msg, sock)
        .await
        .context("When contacting daemon.")
}

pub async fn contact_daemon_or_start_it<M: MessageTrait + 'static>(msg: Message<M>) -> Result<()> {
    if let Err(e) = contact_daemon(msg.clone()).await {
        for cause in e.chain() {
            if let Some(e) = cause.downcast_ref::<tokio::io::Error>() {
                if matches!(e.kind(), ErrorKind::ConnectionRefused) {
                    Command::new("target/debug/dake")
                        .arg("daemon")
                        .spawn()
                        .context("Failed to spawn the daemon.")?;

                    let cloned_msg = msg.clone();
                    let message_sending =
                        spawn(async move { while contact_daemon(msg.clone()).await.is_err() {} });
                    return match timeout(Duration::from_secs(1), message_sending).await {
                        Ok(_) => Ok(()),
                        Err(_) => match contact_daemon(cloned_msg).await {
                            Ok(_) => Ok(()),
                            Err(e) => bail!(
                                "We failed to send the message to the daemon after starting it: {e}"
                            ),
                        },
                    };
                }
            }
        }
        bail!(e)
    }
    Ok(())
}

pub async fn read_next_message(
    tcp_stream: &mut TcpStream,
    kind: MessageKind,
) -> Result<Option<Vec<u8>>> {
    let header_length = MessageHeader::get_header_length();
    let mut header = vec![0; header_length];

    if tcp_stream.read_exact(&mut header).await.is_err() {
        return Ok(None);
    }

    let header: MessageHeader = dec!(header)?;

    let mut message = vec![0; header.size as usize];
    if let Err(e) = tcp_stream.read_exact(&mut message).await {
        if matches!(e.kind(), ErrorKind::UnexpectedEof) {
            bail!("The message size and the header annotated size doesn't match..");
        } else {
            bail!("Error when reading a message.");
        }
    }

    if kind != header.kind {
        bail!("The asked and received kind doesn't match.")
    } else {
        Ok(Some(message))
    }
}
