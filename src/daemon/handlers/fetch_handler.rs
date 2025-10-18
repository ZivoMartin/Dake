use std::{
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
};

use tracing::{debug, info, warn};

use crate::daemon::{
    communication::{
        DaemonMessage, FetcherMessage, Message, MessageCtx, connect, get_daemon_sock, send_message,
        write_message,
    },
    execute_make,
    fs::get_makefile_path,
};

/// Handles a "fetch" request.
/// Log internal errors, sends stderr messages to the client,
/// and reports build failures to the main daemon. It never panics.
pub async fn handle_fetch(
    MessageCtx { pid, client, state }: MessageCtx,
    target: String,
    labeled_path: Option<PathBuf>,
) {
    let daemon_sock = get_daemon_sock();

    // Helper closure to send both a user-facing error message
    // and a `MakeError` to the daemon.
    let forward_error = |user_message: String| async {
        let sock = pid.sock();

        let msg = Message::new(FetcherMessage::Failed, pid.clone(), daemon_sock);

        if let Err(e) = send_message(msg, client).await {
            warn!("Failed to send the Failed message to the fetcher {client}: {e:?}");
        }

        let msg = Message::new(
            DaemonMessage::StderrLog { log: user_message },
            pid.clone(),
            daemon_sock,
        );

        if let Err(e) = send_message(msg, sock).await {
            warn!("Failed to send stderr log to {sock}: {e:?}");
        }

        let msg = Message::new(
            DaemonMessage::MakeError {
                guilty_node: daemon_sock,
                exit_code: 1,
            },
            pid.clone(),
            daemon_sock,
        );

        if let Err(e) = send_message(msg, sock).await {
            warn!("Failed to forward MakeError to {sock}: {e:?}");
        }
    };

    // Macro to both log an internal warning and forward a user-facing error.
    macro_rules! warn_and_forward {
        ($msg:expr) => {{
            warn!($msg);
            forward_error("Dake encountered an internal error.".into()).await;
            return;
        }};
        ($msg:expr, $user:expr) => {{
            warn!($msg);
            forward_error($user.into()).await;
            return;
        }};
    }

    info!("Fetcher started for target '{target}' requested by {client}");

    // --- Step 1: Resolve makefile path ---
    let mut path = match labeled_path.or_else(|| get_makefile_path(&pid).ok()) {
        Some(p) => {
            debug!("Resolved makefile path: {:?}", p);
            p
        }
        None => warn_and_forward!("Failed to resolve the makefile path."),
    };

    // --- Step 2: Fetching args ---
    let args = state
        .read_args(&pid)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            warn!("Failed to read args in the state for the process {pid:?}");
            Vec::new()
        });

    // --- Step 3: Execute make ---
    info!("Running make for target '{target}' at path {:?}", path);
    match execute_make(
        &state,
        pid.clone(),
        path.clone(),
        Some(target.clone()),
        &args,
    )
    .await
    {
        Ok(Some(status)) => {
            let exit_code = status.code().unwrap_or_else(|| {
                warn!("Fetcher: make process terminated by signal (no exit code)");
                0
            });

            if !status.success() {
                warn!("Fetcher: make exited with status {exit_code} for target '{target}'");
                let inner = DaemonMessage::MakeError {
                    guilty_node: get_daemon_sock(),
                    exit_code,
                };

                let msg = Message::new(inner, pid.clone(), pid.sock());
                if let Err(e) = send_message(msg, pid.sock()).await {
                    warn!("Failed to send build failure to {}: {e:?}", pid.sock());
                }
            } else {
                info!("Make completed successfully for target '{target}'");
            }
        }
        Ok(None) => {
            info!("The make process has been aborted.");
            return;
        }
        Err(e) => warn_and_forward!("Failed to start make process for {target}: {e:?}"),
    }

    // --- Step 4: Validate resulting target path ---
    path.push(target.clone());
    debug!("Checking resulting path {:?}", path);

    match path.metadata() {
        Ok(meta) if meta.is_file() => debug!("Verified target file exists: {:?}", path),
        Ok(_) => warn_and_forward!(
            "Resolved path {path:?} is not a file (possibly directory or special entry)",
            format!(
                "The target '{target}' did not produce a file (possibly a directory or other entry)."
            )
        ),
        Err(e) => warn_and_forward!(
            "Failed to access target path {path:?}: {e:?}",
            format!("The target '{target}' does not seem to produce a file. Check your Makefile.")
        ),
    }

    // --- Step 5: Send artifact to client ---
    info!("Opening built artifact at {:?}", path);
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => warn_and_forward!("Failed to open built artifact {path:?}: {e:?}"),
    };

    let mut reader = BufReader::new(file);
    let err = format!(
        "Failed to forward '{target}' from {daemon_sock} to {client}. \
        The Dake daemon on {client} might be down."
    );

    debug!("Connecting to client {client}");
    let mut stream = match connect(client).await {
        Ok(s) => s,
        Err(e) => warn_and_forward!("Failed to connect to client {client}: {e:?}", err),
    };

    info!("Streaming file '{target}' to {client}");
    loop {
        let mut buf = vec![0; 8192];
        let n = match reader.read(&mut buf) {
            Ok(n) => n,
            Err(e) => warn_and_forward!("Failed to read {path:?}: {e:?}", err),
        };
        if n == 0 {
            debug!("End of file reached for '{target}'");
            break;
        }

        let message = Message::new(FetcherMessage::Object(buf), pid.clone(), get_daemon_sock());
        if let Err(e) = write_message(&mut stream, message).await {
            warn_and_forward!("Failed to send packet to {client}: {e:?}", err);
        }
    }

    info!("Fetcher successfully completed for target '{target}'");
}
