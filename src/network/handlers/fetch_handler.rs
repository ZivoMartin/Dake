use std::{
    fs::File,
    io::{BufReader, Read},
    net::SocketAddr,
    path::PathBuf,
    process::Command,
};

use tracing::warn;

use crate::{
    network::{
        FetcherMessage, Message,
        fs::get_makefile_path,
        get_daemon_sock,
        message_ctx::MessageCtx,
        send_message,
        utils::{connect, write_message},
    },
    process_id::ProcessId,
};

pub async fn handle_fetch(
    MessageCtx { pid, client, .. }: MessageCtx,
    target: String,
    labeled_path: Option<PathBuf>,
) {
    fn build_path(
        labeled_path: Option<PathBuf>,
        pid: &ProcessId,
        target: &str,
    ) -> Result<PathBuf, FetcherMessage> {
        // First resolve the path
        let mut path = match labeled_path.or_else(|| get_makefile_path(&pid).ok()) {
            Some(p) => p,
            None => return Err(FetcherMessage::PathResolutionFailed),
        };

        // Run `make`
        let make_status = Command::new("make").arg(target).current_dir(&path).status();

        if make_status.is_err() {
            return Err(FetcherMessage::MakeFailed);
        }

        // Adjust path to point to the target
        path.push(target);

        // Now handle file/dir cases
        if path.is_file() {
            Ok(path)
        } else if path.is_dir() {
            Err(FetcherMessage::IsFolder)
        } else {
            Err(FetcherMessage::FileMissing)
        }
    }

    async fn forward_error(inner: FetcherMessage, pid: ProcessId, client: SocketAddr) {
        let message = Message::new(inner, pid, get_daemon_sock());
        if let Err(e) = send_message(message, client).await {
            warn!("Fetcher: Failed to forward the error to {client} because of {e}");
        }
    }

    match build_path(labeled_path, &pid, &target) {
        Ok(path) => {
            let f = match File::open(path.clone()) {
                Ok(f) => f,
                Err(_) => {
                    forward_error(FetcherMessage::OpenFailed, pid, client).await;
                    return;
                }
            };
            let mut reader = BufReader::new(f);
            let mut stream = match connect(client).await {
                Ok(stream) => stream,
                Err(e) => {
                    warn!("Failed to connect to the client {client} because of {e}.");
                    return;
                }
            };
            loop {
                let mut buf = vec![0; 8192];
                let n = match reader.read(&mut buf) {
                    Ok(n) => n,
                    Err(e) => {
                        warn!("Failed to read in {path:?} because of {e}");
                        forward_error(FetcherMessage::ReadFailed, pid, client).await;
                        break;
                    }
                };
                if n == 0 {
                    break;
                }

                let message =
                    Message::new(FetcherMessage::Object(buf), pid.clone(), get_daemon_sock());
                if let Err(e) = write_message(&mut stream, message).await {
                    warn!("Fetcher: Failed to send a packet to the {client} because of {e}");
                    break;
                }
            }
        }
        Err(err) => {
            forward_error(err, pid, client).await;
        }
    }
}
