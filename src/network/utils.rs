//! # Network Utilities
//!
//! This module provides common networking utilities for the Dake distributed
//! build system.  
//!
//! Responsibilities include:
//! - Sending serialized messages to sockets
//! - Resolving the daemon socket address
//! - Contacting the daemon (and spawning it if necessary)
//! - Reading incoming messages from TCP streams
//!
//! All operations rely on the message protocol defined in [`MessageHeader`] and
//! [`MessageKind`].

use std::{net::SocketAddr, process::Command, time::Duration};

use anyhow::{Context, Result, bail};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ErrorKind},
    net::TcpStream,
    spawn,
    time::timeout,
};
use tracing::{error, info, warn};

use crate::{
    dec, enc,
    network::{
        DEFAULT_SOCK, MessageKind,
        messages::{Message, MessageHeader, MessageTrait},
    },
};

/// Write a message on a given stream.
pub async fn write_message<M: MessageTrait>(
    tcp_stream: &mut TcpStream,
    msg: Message<M>,
) -> Result<()> {
    let enc_msg = MessageHeader::wrap(enc!(msg), msg.get_kind());
    tcp_stream
        .write_all(&enc_msg)
        .await
        .context("When writing on the stream")
}

/// Connect with tcp on the given socket.
pub async fn connect(sock: SocketAddr) -> Result<TcpStream> {
    info!("Utils: Attempting to connect to socket {}.", sock);
    let stream = TcpStream::connect(sock)
        .await
        .context("When connecting on the stream.")?;
    info!("Utils: Connected to {}", sock);
    Ok(stream)
}

/// Sends a serialized message to the given socket.
/// Returns the stream used to send the message.
/// Returns an error if connection or writing fails.
pub async fn send_message<M: MessageTrait>(msg: Message<M>, sock: SocketAddr) -> Result<TcpStream> {
    info!("Utils: Attempting to send a message to socket {}", sock);
    let mut stream = connect(sock).await?;
    write_message(&mut stream, msg).await?;
    info!("Utils: Successfully sent message to {}", sock);
    Ok(stream)
}

/// Returns the default daemon socket address.
pub fn get_daemon_sock() -> SocketAddr {
    DEFAULT_SOCK
}

/// Sends a message directly to the daemon.
/// Returns an error if sending fails.
pub async fn contact_daemon<M: MessageTrait>(msg: Message<M>) -> Result<TcpStream> {
    let sock = get_daemon_sock();
    info!("Utils: Contacting daemon at {}", sock);
    send_message(msg, sock)
        .await
        .context("When contacting daemon.")
}

/// Sends a message to the daemon, starting it if not already running.
///
/// If the daemon is not active (connection refused), this function:
/// - Spawns the daemon (`dake daemon`)
/// - Waits up to 1s for it to start
/// - Retries sending the message
///
/// # Errors
/// Returns an error if the daemon cannot be started or contacted.
pub async fn contact_daemon_or_start_it<M: MessageTrait + 'static>(msg: Message<M>) -> Result<()> {
    if let Err(e) = contact_daemon(msg.clone()).await {
        for cause in e.chain() {
            if let Some(e) = cause.downcast_ref::<tokio::io::Error>() {
                if matches!(e.kind(), ErrorKind::ConnectionRefused) {
                    warn!("Utils: Daemon not running, attempting to spawn it...");

                    Command::new("target/debug/dake")
                        .arg("daemon")
                        .spawn()
                        .context("Failed to spawn the daemon.")?;

                    info!("Utils: Daemon process spawned, waiting for availability...");

                    let cloned_msg = msg.clone();
                    let message_sending =
                        spawn(async move { while contact_daemon(msg.clone()).await.is_err() {} });

                    return match timeout(Duration::from_secs(1), message_sending).await {
                        Ok(_) => {
                            info!("Utils: Daemon is responsive, message sent successfully");
                            Ok(())
                        }
                        Err(_) => match contact_daemon(cloned_msg).await {
                            Ok(_) => {
                                info!("Utils: Retried and successfully sent message to daemon");
                                Ok(())
                            }
                            Err(e) => {
                                error!(
                                    "Utils: Failed to send message to daemon after starting it: {e}"
                                );
                                bail!(
                                    "We failed to send the message to the daemon after starting it: {e}"
                                )
                            }
                        },
                    };
                }
            }
        }
        bail!(e)
    }
    Ok(())
}

/// Reads the next message from a TCP stream.
///
/// This function:
/// 1. Reads a [`MessageHeader`] from the stream.
/// 2. Reads the payload based on the size in the header.
/// 3. Verifies that the expected [`MessageKind`] matches the header.
///
/// # Arguments
/// * `tcp_stream` - The TCP stream to read from.
/// * `kind` - The expected message kind.
///
/// # Returns
/// Returns `Ok(Some(Vec<u8>))` with the raw message payload, or `Ok(None)` if
/// the stream was closed.
///
/// # Errors
/// Returns an error if deserialization fails, if the message size is invalid,
/// or if the message kind does not match.
pub async fn read_next_message(
    tcp_stream: &mut TcpStream,
    kind: MessageKind,
) -> Result<Option<Vec<u8>>> {
    let header_length = MessageHeader::get_header_length();
    let mut header = vec![0; header_length];

    // Read message header
    if tcp_stream.read_exact(&mut header).await.is_err() {
        warn!("Utils: Connection closed while trying to read header");
        return Ok(None);
    }

    let header: MessageHeader = dec!(header)?;
    info!(
        "Utils: Received message header with size={} and kind={:?}",
        header.size, header.kind
    );

    // Read message payload
    let mut message = vec![0; header.size as usize];
    if let Err(e) = tcp_stream.read_exact(&mut message).await {
        if matches!(e.kind(), ErrorKind::UnexpectedEof) {
            error!("Utils: Header size did not match actual message size");
            bail!("The message size and the header annotated size doesn't match.");
        } else {
            error!("Utils: Error when reading message: {}", e);
            bail!("Error when reading a message.");
        }
    }

    // Check message kind
    if kind != header.kind {
        error!(
            "Utils: Expected message kind {:?}, but received {:?}",
            kind, header.kind
        );
        bail!("The asked and received kind doesn't match.");
    } else {
        info!("Utils: Successfully read message of kind {:?}", kind);
        Ok(Some(message))
    }
}
