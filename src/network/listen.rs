use anyhow::{Context, Result};
use log::warn;
use tokio::{net::TcpListener, task::spawn};

use crate::{
    dec,
    network::{
        Message, MessageKind, fetch_handler::handle_fetch, fs::init_fs, get_daemon_sock,
        makefile_receiver::receiv_makefile, messages::DaemonMessage, new_process::new_process,
        utils::read_next_message,
    },
};

pub async fn start() -> Result<()> {
    init_fs()?;

    let listener = TcpListener::bind(get_daemon_sock())
        .await
        .context("When starting the daemon.")?;

    loop {
        let (mut tcp_stream, _) = match listener.accept().await {
            Ok(tcp_stream) => tcp_stream,
            _ => continue,
        };

        spawn(async move {
            loop {
                let message =
                    match read_next_message(&mut tcp_stream, MessageKind::DaemonMessage).await {
                        Ok(Some(msg)) => msg,
                        Ok(None) => break,
                        Err(e) => {
                            warn!("{}", e.root_cause());
                            break;
                        }
                    };
                let message: Message<DaemonMessage> = match dec!(message) {
                    Ok(msg) => msg,
                    _ => {
                        warn!("Failed to decrypt DaemonMessage.");
                        break;
                    }
                };

                spawn(async move {
                    let pid = message.pid;
                    let client = message.client;
                    match message.inner {
                        DaemonMessage::NewProcess { makefiles, args } => {
                            new_process(pid, client, makefiles, args).await
                        }
                        DaemonMessage::Distribute { makefile } => {
                            receiv_makefile(pid, client, makefile).await
                        }
                        DaemonMessage::Fetch {
                            target,
                            labeled_path,
                        } => handle_fetch(pid, client, target, labeled_path).await,
                    }
                });
            }
        });
    }
}
