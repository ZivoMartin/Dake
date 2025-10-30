//! # Network Utilities
//!
//! This module provides common networking utilities for the Dake distributed
//! build system.  

use std::{
    env::var,
    io::ErrorKind,
    net::{IpAddr, UdpSocket},
    path::PathBuf,
    process::Command,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    spawn,
    time::timeout,
};
use tracing::{error, info, warn};

use crate::{
    dec, enc,
    env_variables::EnvVariable,
    network::{
        DAEMON_UNIX_SOCKET, DEFAULT_PORT, Message, MessageHeader, MessageKind, MessageTrait,
        SocketAddr, Stream,
    },
    utils::get_dake_path,
};

/// Write a message on a given stream.
pub async fn write_message<M: MessageTrait, S: AsyncWriteExt + Unpin>(
    stream: &mut S,
    msg: Message<M>,
) -> Result<()> {
    info!("Writing a new message : {:?}", msg.get_kind());

    let enc_msg = MessageHeader::wrap(enc!(msg)?, msg.get_kind())
        .context("Failed to compute the message header.")?;
    stream
        .write_all(&enc_msg)
        .await
        .context("When writing on the stream")?;
    stream
        .flush()
        .await
        .context("Failed to flush on the stream")
}

/// Connect with tcp on the given socket.
pub async fn connect(sock: SocketAddr) -> Result<Stream> {
    info!("Attempting to connect to socket {}.", sock);
    let stream = Stream::connect(sock.clone())
        .await
        .context("When connecting on the stream.")?;

    info!("Connected to {}", sock);
    Ok(stream)
}

/// Sends a serialized message to the given socket.
/// Returns the stream used to send the message.
/// Returns an error if connection or writing fails.
pub async fn send_message<M: MessageTrait>(msg: Message<M>, sock: SocketAddr) -> Result<Stream> {
    info!("Attempting to send a message to socket {}", sock);
    let mut stream = connect(sock.clone()).await?;
    write_message(&mut stream, msg).await?;
    info!("Successfully sent message to {}", sock);
    Ok(stream)
}

pub fn get_daemon_port() -> u16 {
    var(EnvVariable::DaemonPort.to_string())
        .context("Failed to get the daemon port with environment variable.")
        .and_then(|port| {
            let err = format!(
                "Failed to parse the content of {} as an integer.",
                EnvVariable::DaemonPort
            );
            let res = port.parse::<u16>().context(err.clone());
            if let Err(e) = &res {
                warn!("{err} {e}");
            }
            res
        })
        .unwrap_or(DEFAULT_PORT)
}

pub fn get_daemon_ip() -> Result<IpAddr> {
    var(EnvVariable::DaemonIp.to_string())
        .context("Failed to get the ip with environment variable.")
        .and_then(|ip| {
            let err = format!(
                "Failed to parse the content of {} as an ip.",
                EnvVariable::DaemonIp
            );
            let res = ip.parse::<IpAddr>().context(err.clone());
            if let Err(e) = &res {
                warn!("{err} {e}");
            }
            res
        })
        .or_else(|_| {
            let socket = UdpSocket::bind("0.0.0.0:0")
                .context("Failed to bind on udp to get the default daemon address.")?;
            socket.connect("8.8.8.8:80")?;
            Ok(socket
                .local_addr()
                .context("Failed to fetch local address on the UDP socket.")?
                .ip())
        })
}

/// Returns the daemon's TCP socket address based on environment variables
/// or defaults. If IP is missing, returns an error.
/// If port is missing, uses DEFAULT_PORT.
pub fn get_daemon_tcp_sock() -> Result<SocketAddr> {
    let ip = get_daemon_ip()?;
    let port: u16 = get_daemon_port();
    Ok(SocketAddr::new_tcp(ip, port))
}

/// Returns the daemon socket address using the DAEMON_UNIX_SOCKET constant.
pub fn get_daemon_unix_sock() -> Result<SocketAddr> {
    SocketAddr::new_unix(PathBuf::from(DAEMON_UNIX_SOCKET))
}

/// Connect to the daemon, starting it if not already running.
///
/// If the daemon is not active (connection refused), this function:
/// - Spawns the daemon (`dake daemon`)
/// - Waits up to 1s for it to start
/// - Retries connection
///
/// # Errors
/// Returns an error if the daemon cannot be started or contacted.
#[tracing::instrument]
pub async fn connect_with_daemon_or_start_it(daemon_addr: SocketAddr) -> Result<Stream> {
    match connect(daemon_addr.clone()).await {
        Ok(stream) => Ok(stream),
        Err(e) => {
            for cause in e.chain() {
                if let Some(e) = cause.downcast_ref::<tokio::io::Error>() {
                    if matches!(e.kind(), ErrorKind::ConnectionRefused | ErrorKind::NotFound) {
                        info!("Daemon not running, attempting to spawn it...");

                        Command::new(
                            get_dake_path()
                                .context("Failed to fetch dake path when starting daemon.")?,
                        )
                        .arg("daemon")
                        .spawn()
                        .context("Failed to spawn the daemon.")?;

                        info!("Daemon process spawned, waiting for availability...");

                        let thread_daemon_addr = daemon_addr.clone();
                        let connections = spawn(async move {
                            loop {
                                if let Ok(stream) = connect(thread_daemon_addr.clone()).await {
                                    break stream;
                                }
                            }
                        });

                        return match timeout(Duration::from_secs(1), connections).await {
                            Ok(Ok(stream)) => {
                                info!("Daemon is responsive, connected successfully");
                                Ok(stream)
                            }
                            _ => match connect(daemon_addr).await {
                                Ok(stream) => {
                                    info!("Retried and successfully connected to the daemon");
                                    Ok(stream)
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to connect to the daemon after starting it: {e}"
                                    );
                                    bail!("Failed to connect to the daemon after starting it: {e}")
                                }
                            },
                        };
                    } else {
                        warn!("Failed to connect to daemon for a non expected reason: {e:?}");
                    }
                }
            }
            bail!(e)
        }
    }
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
pub async fn read_next_message<S: AsyncReadExt + Unpin>(
    stream: &mut S,
    kind: MessageKind,
) -> Result<Option<Vec<u8>>> {
    let header_length =
        MessageHeader::get_header_length().context("Failed to compute header length.")?;
    let mut header = vec![0; header_length];

    // Read message header
    if stream.read_exact(&mut header).await.is_err() {
        info!("Connection closed while trying to read header");
        return Ok(None);
    }
    info!("Just read a new message on the stream.");

    let header: MessageHeader = dec!(header).context("Failed to decode the MessageHeader.")?;

    info!(
        "Received message header with size={} and kind={:?}",
        header.size, header.kind
    );

    // Read message payload
    let mut message = vec![0; header.size as usize];
    if let Err(e) = stream.read_exact(&mut message).await {
        if matches!(e.kind(), ErrorKind::UnexpectedEof) {
            error!("Header size did not match actual message size");
            bail!("The message size and the header annotated size doesn't match.");
        } else {
            error!("Error when reading message: {}", e);
            bail!("Error when reading a message.");
        }
    }

    // Check message kind
    if kind != header.kind {
        error!(
            "Expected message kind {:?}, but received {:?}",
            kind, header.kind
        );
        bail!("The asked and received kind doesn't match.");
    } else {
        info!("Successfully read message of kind {:?}", kind);
        Ok(Some(message))
    }
}
