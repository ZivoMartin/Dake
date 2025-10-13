use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    net::SocketAddr,
    path::PathBuf,
};

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use crate::{
    dec,
    network::{
        DaemonMessage, FetcherMessage, Message, MessageKind, read_next_message, send_message,
    },
    process_id::ProcessId,
};

pub async fn fetch(
    target: String,
    labeled_path: Option<PathBuf>,
    caller_path: PathBuf,
    sock: SocketAddr,
) -> Result<()> {
    let pid = ProcessId::new(sock, caller_path);
    info!(
        "Fetcher: Starting fetch for target '{}' with pid {:?}",
        target, pid
    );

    // Step 1: Bind a temporary socket for fetcher responses
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("When starting the fetcher socket.")?;
    let fetcher_addr = listener
        .local_addr()
        .context("When requesting the fetcher socket address")?;
    info!("Fetcher: Listening for responses on {}", fetcher_addr);

    // Step 2: Send Fetch request to the remote daemon
    let message = Message::new(
        DaemonMessage::Fetch {
            target: target.clone(),
            labeled_path,
        },
        pid.clone(),
        fetcher_addr,
    );

    info!(
        "Fetcher: Sending Fetch request for '{}' to {}",
        target, sock
    );
    send_message(message, sock).await?;
    info!("Fetcher: Fetch request sent successfully to {}", sock);

    // Step 3: Wait for responses from the daemon
    let mut tcp_stream = loop {
        // Accept connection from the daemon
        match listener.accept().await {
            Ok((tcp_stream, remote_sock)) => {
                if remote_sock == sock {
                    info!("Fetcher: Accepted connection from daemon {}", remote_sock);
                    break tcp_stream;
                } else {
                    warn!(
                        "Fetcher: Unexpected connection - expected {}, got {}",
                        sock, remote_sock
                    );
                }
            }
            Err(e) => {
                warn!("Fetcher: Failed to accept connection: {e}");
            }
        };
    };
    // Step 4: Receiv all the messages and write in the object file output
    let f = OpenOptions::new().create(true).write(true).open(target)?;
    let mut writer = BufWriter::new(f);

    loop {
        let msg = match read_next_message(&mut tcp_stream, MessageKind::FetcherMessage).await {
            Ok(Some(msg)) => {
                info!("Fetcher: Received raw FetcherMessage from {}", sock);
                msg
            }
            Ok(None) => {
                warn!("Fetcher: Connection closed before receiving a message");
                break;
            }
            Err(e) => {
                warn!("Fetcher: Failed to read FetcherMessage: {e}");
                continue;
            }
        };

        // Deserialize FetcherMessage
        let msg: FetcherMessage = dec!(msg)?;
        match msg {
            FetcherMessage::Object(obj) => {
                info!("Fetcher: Received object from {}", sock);
                writer.write(&obj)?;
                // To avoid breaking after the match.
                continue;
            }
            FetcherMessage::OpenFailed => {
                error!("Remote host {sock} failed to open target file after make.")
            }
            FetcherMessage::IsFolder => {
                error!("The make output produced a folder on the remote host {sock}.")
            }
            FetcherMessage::FileMissing => {
                error!(
                    "The make output did not produce a file with the target name on the remote host {sock}."
                )
            }
            FetcherMessage::MakeFailed => error!("The make failed on the remote host {sock}."),
            FetcherMessage::PathResolutionFailed => {
                error!("Failed to resolve the path on the remote host {sock}.")
            }
            FetcherMessage::ReadFailed => {
                error!("The remote host {sock} failed to read the object file.")
            }
        }
        break;
    }

    info!("Fetcher: Finished execution for pid {:?}", pid);
    Ok(())
}
