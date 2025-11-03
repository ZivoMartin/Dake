use anyhow::{Context, Result};
use std::{
    fs::read_to_string,
    path::PathBuf,
    process::{ExitStatus, Stdio},
};
use tokio::{
    io::{AsyncReadExt, BufReader},
    process::Command,
    select, spawn,
    task::JoinHandle,
};
use tracing::{error, info, warn};

use crate::{
    daemon::{Notif, State},
    lock,
    makefile::RemoteMakefile,
    network::{DaemonMessage, Message, SocketAddr, connect, write_message},
    process_id::ProcessId,
};

/// Executes a `make` command asynchronously, forwarding logs to the daemon and
/// reacting to cancellation messages.
///
/// # Behavior
/// 1. Spawns a `make` process in the given working directory.
/// 2. Forwards its `stdout` and `stderr` lines asynchronously to the daemon.
/// 3. Waits for process completion or external `Notif::Done` signal.
/// 4. Returns the process exit status (or `None` if killed early).
///
/// # Returns
/// - `Ok(Some(exit_status))` when the process completes normally.  
/// - `Ok(None)` if it was killed due to a `Notif::Done`.  
/// - `Err(anyhow::Error)` if any I/O, spawn, or await operation failed.
///
/// # Logging
/// This function produces detailed logs for each stage of process management,
/// including line forwarding, signal handling, and subscription setup.
pub async fn execute_make(
    state: &State,
    pid: ProcessId,
    current_dir: PathBuf,
    target: Option<String>,
    args: &[String],
) -> Result<Option<ExitStatus>> {
    info!(
        "Starting make execution for PID {:?} in {:?}",
        pid, current_dir
    );

    if let Some(target) = &target {
        state
            .lock_target(pid.project_id.clone(), target.clone())
            .await
            .context("Failed to lock the target before executing make")?
    }

    let path = RemoteMakefile::guess_path(current_dir.clone())
        .context(format!("There is no makefile at {}", current_dir.display()))?;
    info!("Full path to the makefile: {}", path.display());
    let content = read_to_string(path.clone()).context(format!(
        "Failed to read the content of the makefile at {}",
        path.display(),
    ))?;
    info!("Content of the makefile:\n{content}");

    let caller_sock = state
        .read_process_data(&pid)
        .await
        .context("Failed to fetch the caller sock.")?
        .context("Failed to fetch the caller sock, process is over.")?
        .caller_daemon;

    info!("Just fetched caller_sock: {caller_sock}");

    // --- Step 1: Configure and spawn process ---
    info!("Spawning make process..");

    let mut cmd = Command::new("make");

    if let Some(target) = &target {
        if !target.is_empty() {
            cmd.arg(target);
        }
    }

    cmd.args(args)
        .current_dir(&current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut process = cmd
        .spawn()
        .with_context(|| format!("Failed to spawn make process in {current_dir:?}"))?;

    info!("Spawned make process (pid={:?})", process.id());

    // --- Step 2: Log forwarding helpers ---
    fn spawn_log_forwarder<R, F>(
        pid: ProcessId,
        mut pipe: R,
        make_msg: F,
        caller_sock: SocketAddr,
    ) -> JoinHandle<()>
    where
        R: AsyncReadExt + Unpin + Send + 'static,
        F: Fn(String) -> DaemonMessage + Send + Sync + 'static,
    {
        spawn(async move {
            let mut buf = [0u8; 4096];
            let mut stream = match connect(caller_sock.clone()).await {
                Ok(stream) => stream,
                Err(e) => {
                    warn!("Failed to connect with the daemon: {e}");
                    return;
                }
            };
            loop {
                match pipe.read(&mut buf).await {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buf[..n]);
                        let msg = Message::new(make_msg(text.to_string()), pid.clone());
                        if let Err(e) = write_message(&mut stream, msg).await {
                            warn!("Failed to forward process log to the caller: {e:?}");
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Error reading process output for {:?}: {e:?}", pid);
                        break;
                    }
                }
            }

            info!("Log forwarder terminated for {:?}", pid);
        })
    }

    // --- Step 3: Attach log handlers ---
    let mut handlers = Vec::new();

    if let Some(stdout) = process.stdout.take().map(BufReader::new) {
        info!("Attaching stdout log handler for {:?}", pid);
        handlers.push(spawn_log_forwarder(
            pid.clone(),
            stdout,
            |log| DaemonMessage::StdoutLog { log },
            caller_sock.clone(),
        ));
    } else {
        warn!("Failed to attach stdout for process {:?}", pid);
    }

    if let Some(stderr) = process.stderr.take().map(BufReader::new) {
        info!("Attaching stderr log handler for {:?}", pid);
        handlers.push(spawn_log_forwarder(
            pid.clone(),
            stderr,
            |log| DaemonMessage::StderrLog { log },
            caller_sock,
        ));
    } else {
        warn!("Failed to attach stderr for process {:?}", pid);
    }

    // --- Step 4: Subscribe to notifier hub for process cancellation ---
    info!("Subscribing to notifier hub for PID {:?}", pid);
    let subscriber = {
        let notifier_hub = state.notifier_hub().clone();
        match lock!(notifier_hub).await {
            Ok(mut notifier_hub) => Some(notifier_hub.subscribe(&pid, 100)),
            Err(e) => {
                warn!("Failed to lock notifier_hub: {e}");
                None
            }
        }
    };

    // --- Step 5: Monitor process completion and external signals ---
    let status = match subscriber {
        Some(mut subscriber) => {
            info!("Listening for process completion or Done notification...");
            loop {
                select! {
                    result = process.wait() => {
                        info!("Make process exited normally");
                        break result;
                    }
                    notif = subscriber.recv() => {
                        match notif {
                            Some(n) => {
                                info!("Received notification: {:?}", n);
                                if matches!(n.as_ref(), Notif::Done) {
                                    info!("Received Done signal for {:?}, terminating make process", pid);
                                    if let Err(e) = process.kill().await {
                                        error!("Failed to kill make process for {:?}: {e:?}", pid);
                                    }
                                    return Ok(None);
                                }
                            }
                            None => {
                                warn!("Notifier channel closed unexpectedly");
                                break process.wait().await;
                            }
                        }
                    }
                }
            }
        }
        None => {
            warn!("No subscriber available; waiting for make to finish normally");
            process.wait().await
        }
    };

    // --- Step 6: Return process exit status ---
    let exit_status = status.context("Failed while waiting for make process to finish")?;
    info!(
        "Make process for PID {:?} exited with status: {}",
        pid,
        exit_status.code().unwrap_or(-1)
    );

    // Await all log handlers asynchronously (fire-and-forget)
    for handle in handlers {
        if let Err(e) = handle.await {
            warn!("One of the log handlers panicked or failed: {e:?}");
        }
    }

    if let Some(target) = target {
        state
            .unlock_target(pid.project_id, target)
            .await
            .context("Failed to unlock the target after executing make")?
    }

    Ok(Some(exit_status))
}
