use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result};
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::{
    dec,
    network::{
        DaemonMessage, FetcherMessage, Message, MessageKind, SocketAddr, connect,
        read_next_message, write_message,
    },
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
#[tracing::instrument]
pub async fn fetch(
    target: String,
    labeled_path: Option<PathBuf>,
    caller_path: PathBuf,
    caller_sock: SocketAddr,
    id: u64,
    sock: SocketAddr,
) -> Result<()> {
    let pid = ProcessId::new(id, sock.clone(), caller_path);
    info!("Fetcher started for target '{}' with PID {:?}", target, pid);

    // --- Step 1: Connect to remote daemon ---
    info!("Connecting with the daemon...");
    let mut stream = connect(caller_sock)
        .await
        .context("Failed to connect with the daemon.")?;
    info!("Connected successfully.");

    // --- Step 2: Send Fetch request to remote daemon ---
    let fetch_message = Message::new(
        DaemonMessage::Fetch {
            target: target.clone(),
            labeled_path,
        },
        pid.clone(),
    );

    info!("Sending Fetch request for '{}' to {}", target, sock);
    write_message(&mut stream, fetch_message)
        .await
        .with_context(|| format!("Failed to send Fetch request for '{target}' to {sock}"))?;
    info!(
        "Fetch request for '{}' sent successfully to {}",
        target, sock
    );

    // --- Step 3: Receive all messages and write object to file ---
    let file_path = PathBuf::from(&target);
    info!("Opening output file at {:?}", file_path);

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&file_path)
        .with_context(|| format!("Failed to open output file for target '{target}'"))?;
    let mut writer = BufWriter::new(file);

    info!("Waiting for object data from daemon {}", sock);

    loop {
        let msg = match read_next_message(&mut stream, MessageKind::FetcherMessage).await {
            Ok(Some(raw_msg)) => {
                info!("Received raw FetcherMessage from {}", sock);
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
                info!("Writing {} bytes from object chunk to file", obj.len());
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
                // Wait for parent to send Done notification before terminating
                // This prevents premature process cleanup that could cause spurious errors
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
