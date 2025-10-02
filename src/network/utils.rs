use std::{
    net::{IpAddr, SocketAddr},
    process::Command,
    time::Duration,
};

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
        DEFAULT_ADDR, DEFAULT_PORT, MessageKind,
        messages::{Message, MessageHeader},
    },
};

pub fn parse_raw_ip(raw_ip: &str) -> Result<SocketAddr> {
    Ok(match raw_ip.parse() {
        Ok(sa) => sa,
        Err(_) => {
            let ip: IpAddr = raw_ip
                .parse()
                .with_context(|| format!("Invalid IP string: {raw_ip}"))?;
            SocketAddr::new(ip, DEFAULT_PORT)
        }
    })
}

pub async fn send_message<M: Message>(msg: M, sock: SocketAddr) -> Result<()> {
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

pub fn get_deamon_address() -> SocketAddr {
    SocketAddr::new(DEFAULT_ADDR, DEFAULT_PORT)
}

pub async fn contact_deamon<M: Message>(msg: M) -> Result<()> {
    let sock = get_deamon_address();
    send_message(msg, sock)
        .await
        .context("When contacting deamon.")
}

pub async fn contact_deamon_or_start_it<M: Message + 'static>(msg: M) -> Result<()> {
    if let Err(e) = contact_deamon(msg.clone()).await {
        for cause in e.chain() {
            if let Some(e) = cause.downcast_ref::<tokio::io::Error>() {
                if matches!(e.kind(), ErrorKind::ConnectionRefused) {
                    Command::new("target/debug/dake")
                        .arg("deamon")
                        .spawn()
                        .context("Failed to spawn the deamon.")?;

                    let cloned_msg = msg.clone();
                    let message_sending =
                        spawn(async move { while contact_deamon(msg.clone()).await.is_err() {} });
                    return match timeout(Duration::from_secs(1), message_sending).await {
                        Ok(_) => Ok(()),
                        Err(_) => match contact_deamon(cloned_msg).await {
                            Ok(_) => Ok(()),
                            Err(e) => bail!(
                                "We failed to send the message to the deamon after starting it: {e}"
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
