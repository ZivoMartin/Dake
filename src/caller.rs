use std::{env::current_dir, fs::write, time::Duration};

use crate::{
    lexer::guess_path_and_lex, makefile::RemoteMakefileSet, network::Message, process_id::ProcessId,
};
use anyhow::{Context, Result};
use log::warn;
use tokio::{net::TcpListener, time::timeout};

use crate::{
    dec,
    network::{
        DaemonMessage, MessageKind, ProcessMessage, contact_daemon_or_start_it, get_daemon_sock,
        read_next_message,
    },
};

const TMP_MAKEFILE_NAME: &'static str = "dake_tmp_makefile";

pub async fn make(mut args: Vec<String>) -> Result<()> {
    let caller_dir = current_dir()?;
    let tokens = guess_path_and_lex()?;
    let makefiles = RemoteMakefileSet::generate(tokens, caller_dir.clone(), get_daemon_sock());
    let daemon_sock = get_daemon_sock();

    write(TMP_MAKEFILE_NAME, makefiles.my_makefile())?;

    args.append(&mut vec![
        String::from("--file"),
        String::from(TMP_MAKEFILE_NAME),
    ]);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("When starting the caller socket.")?;
    let caller_addr = listener
        .local_addr()
        .context("When requesting the caller socket address")?;

    let message = Message::new(
        DaemonMessage::NewProcess {
            makefiles: makefiles.drop_makefiles(),
            args,
        },
        ProcessId::new(daemon_sock, caller_dir),
        caller_addr,
    );

    contact_daemon_or_start_it(message).await?;

    let timer = timeout(Duration::from_secs(1), async move {
        let daemon_addr = daemon_sock.ip();
        loop {
            match listener.accept().await {
                Ok((tcp_stream, addr)) => {
                    let ip = addr.ip();
                    if ip == daemon_addr {
                        break tcp_stream;
                    } else {
                        warn!(
                            "The caller should only receiv messages from the daemon. Was waiting for {daemon_addr}, received {ip}."
                        );
                    }
                }
                Err(e) => {
                    warn!("The listener failed to accept a connection due to {e}.")
                }
            }
        }
    });

    if let Ok(mut tcp_stream) = timer.await {
        loop {
            let msg = match read_next_message(&mut tcp_stream, MessageKind::ProcessMessage).await {
                Ok(Some(msg)) => msg,
                Ok(None) => {
                    warn!(
                        "Caller has been closed naturally, but should be closed through End message."
                    );
                    break;
                }
                Err(e) => {
                    warn!("Failed to receiv a message from the caller: {e}",);
                    continue;
                }
            };

            let msg: ProcessMessage = dec!(msg)?;
            match msg {
                ProcessMessage::End => {
                    break;
                }
            }
        }
    } else {
        warn!("The daemon is not responding.")
    }
    Ok(())
}
