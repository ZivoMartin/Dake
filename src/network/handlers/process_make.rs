use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, BufReader},
    process::Command,
    spawn,
    task::JoinHandle,
};
use tracing::warn;

use crate::{
    network::{DaemonMessage, Message, ProcessMessage, get_daemon_sock, send_message},
    process_id::ProcessId,
};

pub async fn execute_make(
    pid: ProcessId,
    current_dir: String,
    target: Option<String>,
    args: &[String],
) -> Result<()> {
    let mut cmd = Command::new("make");

    if let Some(target) = target {
        if !target.is_empty() {
            cmd.arg(target);
        }
    }

    cmd.args(args)
        .current_dir(current_dir.clone())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut process = cmd
        .spawn()
        .context(format!("Failed to execute make in {current_dir}."))?;

    fn handle_logging<R, F>(pid: ProcessId, pipe: R, cast: F) -> JoinHandle<()>
    where
        R: AsyncBufRead + Unpin + Send + 'static,
        F: Fn(String) -> ProcessMessage + Send + 'static,
    {
        spawn(async move {
            let client = get_daemon_sock();
            let mut lines = pipe.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let msg = Message::new(cast(line), pid.clone(), client);
                if let Err(e) = send_message(msg, pid.sock).await {
                    warn!("Failed to send the log to {} because of {e}.", pid.sock);
                }
            }
        })
    }

    let mut jhs = Vec::with_capacity(2);

    if let Some(stdout) = process.stdout.take().map(BufReader::new) {
        jhs.push(handle_logging(pid.clone(), stdout, |log| {
            ProcessMessage::StdoutLog { log }
        }));
    } else {
        warn!("Failed to take stdout for the process {pid:?}");
    }

    if let Some(stderr) = process.stderr.take().map(BufReader::new) {
        jhs.push(handle_logging(pid.clone(), stderr, |log| {
            ProcessMessage::StderrLog { log }
        }));
    } else {
        warn!("Failed to take stderr for the process {pid:?}");
    }

    for jh in jhs {
        jh.await
            .unwrap_or_else(|e| warn!("One of the log handler failed because of {e}."))
    }

    let status = process
        .wait()
        .await
        .context("Failed to wait for the process.")?;

    let exit_code = status.code().unwrap_or_else(|| {
        warn!("Failed to fetch exit code in the output status.");
        0
    }) as u32;

    if !status.success() {
        let inner = DaemonMessage::MakeError {
            guilty_node: get_daemon_sock(),
            exit_code,
        };
        let msg = Message::new(inner, pid.clone(), pid.sock);
        if let Err(e) = send_message(msg, pid.sock).await {
            warn!("Failed to send the log to {} because of {e}.", pid.sock);
        }
    }

    Ok(())
}
