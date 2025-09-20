use std::{env::current_dir, fs::write, process::Command, thread::spawn, time::Duration};

use anyhow::{Context, Result};
use dake::lexer::{Makefile, guess_path_and_lex};
use log::warn;
use tokio::{net::TcpListener, time::timeout};

use crate::{
    dec,
    network::{
        DeamonMessage, MessageKind, ProcessMessage, RemoteMakefile, contact_deamon_or_start_it,
        get_deamon_address, read_next_message,
    },
};

const TMP_MAKEFILE_NAME: &'static str = "dake_tmp_makefile";

pub async fn make(mut args: Vec<String>) -> Result<()> {
    let makefile: Makefile = guess_path_and_lex()?;
    let makefiles = makefile.generate("my ip".to_string());

    write(TMP_MAKEFILE_NAME, makefiles.my_makefile())?;

    let path = current_dir()?;

    let makefiles = makefiles
        .remote_makefiles()
        .into_iter()
        .map(|m| RemoteMakefile::cast_remote_makefile(m.clone(), path.clone()))
        .collect::<Result<Vec<RemoteMakefile>>>()?;

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

    let message = DeamonMessage::NewProcess {
        makefiles,
        caller_addr,
        args,
        entry_makefile_dir: current_dir()?,
    };

    contact_deamon_or_start_it(message).await?;

    let timer = timeout(Duration::from_secs(1), async move {
        let deamon_addr = get_deamon_address().ip();
        loop {
            match listener.accept().await {
                Ok((tcp_stream, addr)) => {
                    let ip = addr.ip();
                    if ip == deamon_addr {
                        break tcp_stream;
                    } else {
                        warn!(
                            "The caller should only receiv messages from the deamon. Was waiting for {deamon_addr}, received {ip}."
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
                    warn!(
                        "Failed to receiv a message from the caller: {}",
                        e.root_cause()
                    );
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
        warn!("The deamon is not responding.")
    }
    Ok(())
}
