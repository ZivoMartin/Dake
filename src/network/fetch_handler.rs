use std::{net::SocketAddr, path::PathBuf};

use crate::process_id::ProcessId;

pub async fn handle_fetch(
    _pid: ProcessId,
    _client: SocketAddr,
    _target: String,
    _labeled_path: Option<PathBuf>,
) {
    todo!()
    // Command::new("make")
    //     .arg(target)
    //     .current_dir()
    //     .status()
    // .unwrap();
}
