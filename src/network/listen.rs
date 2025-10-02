use anyhow::{Context, Result};
use log::warn;
use tokio::{net::TcpListener, task::spawn};

use crate::{
    dec,
    network::{
        MessageKind, fetch_handler::handle_fetch, fs::init_fs, get_deamon_address,
        makefile_receiver::receiv_makefile, messages::DeamonMessage, new_process::new_process,
        utils::read_next_message,
    },
};

pub async fn start() -> Result<()> {
    init_fs()?;

    let listener = TcpListener::bind(get_deamon_address())
        .await
        .context("When starting the deamon.")?;

    loop {
        let (mut tcp_stream, _) = match listener.accept().await {
            Ok(tcp_stream) => tcp_stream,
            _ => continue,
        };

        spawn(async move {
            loop {
                let message =
                    match read_next_message(&mut tcp_stream, MessageKind::DeamonMessage).await {
                        Ok(Some(msg)) => msg,
                        Ok(None) => break,
                        Err(e) => {
                            warn!("{}", e.root_cause());
                            break;
                        }
                    };
                let message: DeamonMessage = match dec!(message) {
                    Ok(msg) => msg,
                    _ => {
                        warn!("Failed to decrypt DeamonMessage.");
                        break;
                    }
                };

                spawn(async move {
                    match message {
                        DeamonMessage::NewProcess {
                            makefiles,
                            caller_addr,
                            entry_makefile_dir,
                            args,
                        } => new_process(makefiles, caller_addr, entry_makefile_dir, args).await,
                        DeamonMessage::Distribute(makefile, path) => {
                            receiv_makefile(makefile, path).await
                        }
                        DeamonMessage::Fetch { target, sock } => handle_fetch(target, sock).await,
                    }
                });
            }
        });
    }
}
