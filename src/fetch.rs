use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    net::SocketAddr,
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result};
use tokio::{net::TcpListener, time::sleep};
use tracing::{debug, error, info, warn};

use crate::{
    daemon::communication::{
        DaemonMessage, FetcherMessage, Message, MessageKind, read_next_message, send_message,
    },
    dec,
    process_id::ProcessId,
};

/// Handles the client-side of a fetch operation.
///
/// This function:
/// 1. Spawns a temporary TCP listener for daemon responses.
/// 2. Sends a `Fetch` request to the remote daemon.
/// 3. Accepts a connection from the daemon.
/// 4. Receives and writes `FetcherMessage::Object` data into a target file.
/// It is the *mirror* of the daemon’s `handle_fetch()` operation.
///
/// # Notes
/// This function does not stop on daemon-side errors immediately — it may wait
/// after a `FetcherMessage::Failed` to allow parent synchronization.
pub async fn fetch(
    target: String,
    labeled_path: Option<PathBuf>,
    caller_path: PathBuf,
    id: u64,
    sock: SocketAddr,
) -> Result<()> {
    let pid = ProcessId::new(id, sock, caller_path);
    info!("Fetcher started for target '{}' with PID {:?}", target, pid);

    // --- Step 1: Bind temporary listener for fetcher responses ---
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("Failed to start the fetcher listener socket")?;
    let fetcher_addr = listener
        .local_addr()
        .context("Failed to retrieve fetcher socket address")?;
    info!("Fetcher listening for responses on {}", fetcher_addr);

    // --- Step 2: Send Fetch request to remote daemon ---
    let fetch_message = Message::new(
        DaemonMessage::Fetch {
            target: target.clone(),
            labeled_path,
        },
        pid.clone(),
        fetcher_addr,
    );

    info!("Sending Fetch request for '{}' to {}", target, sock);
    send_message(fetch_message, sock)
        .await
        .with_context(|| format!("Failed to send Fetch request for '{target}' to {sock}"))?;
    info!(
        "Fetch request for '{}' sent successfully to {}",
        target, sock
    );

    // --- Step 3: Wait for daemon connection ---
    let mut tcp_stream = loop {
        match listener.accept().await {
            Ok((stream, remote_sock)) => {
                if remote_sock == sock {
                    info!("Accepted connection from expected daemon {}", remote_sock);
                    break stream;
                } else {
                    warn!(
                        "Unexpected connection: expected {}, got {} — ignoring",
                        sock, remote_sock
                    );
                }
            }
            Err(e) => {
                warn!("Failed to accept connection on fetcher listener: {e:?}");
            }
        }
    };

    // --- Step 4: Receive all messages and write object to file ---
    let file_path = PathBuf::from(&target);
    debug!("Opening output file at {:?}", file_path);

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&file_path)
        .with_context(|| format!("Failed to open output file for target '{target}'"))?;
    let mut writer = BufWriter::new(file);

    info!("Waiting for object data from daemon {}", sock);

    loop {
        let msg = match read_next_message(&mut tcp_stream, MessageKind::FetcherMessage).await {
            Ok(Some(raw_msg)) => {
                debug!("Received raw FetcherMessage from {}", sock);
                raw_msg
            }
            Ok(None) => {
                warn!(
                    "Connection closed by daemon {} before message received",
                    sock
                );
                break;
            }
            Err(e) => {
                warn!("Failed to read FetcherMessage from {}: {e:?}", sock);
                continue;
            }
        };

        let msg: FetcherMessage = dec!(msg)?;
        match msg {
            FetcherMessage::Object(obj) => {
                debug!("Writing {} bytes from object chunk to file", obj.len());
                writer
                    .write_all(&obj)
                    .with_context(|| format!("Failed writing object data for target '{target}'"))?;
            }
            FetcherMessage::Failed => {
                error!(
                    "Fetcher: Daemon reported a fetch failure for target '{}'. \
                     Waiting for parent synchronization before exit...",
                    target
                );
                // Synchronization delay before termination
                sleep(Duration::from_secs(90)).await;
            }
        }
    }

    writer
        .flush()
        .context("Failed to flush file buffer after receiving all data")?;

    info!("Fetcher finished successfully for PID {:?}", pid);
    Ok(())
}
