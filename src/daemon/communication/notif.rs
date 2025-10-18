use std::net::SocketAddr;

use crate::daemon::handlers::OutputFile;

#[derive(Debug)]
pub enum Notif {
    Done,
    Log {
        output: OutputFile,
        log: String,
    },
    Error {
        exit_code: i32,
        guilty_node: SocketAddr,
    },
}
