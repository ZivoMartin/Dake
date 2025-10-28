use crate::{daemon::handlers::OutputFile, network::SocketAddr};
use tracing::debug;

/// Notification message exchanged between threads (e.g., daemon and workers).
#[derive(Debug)]
pub enum Notif {
    /// Task successfully completed.
    Done,

    /// Log message produced during execution.
    Log { output: OutputFile, log: String },

    /// Fatal error with exit code and the node responsible.
    Error {
        exit_code: i32,
        guilty_node: SocketAddr,
    },
}

impl Notif {
    /// Emits a trace log when a notification is sent or handled.
    pub fn trace(&self) {
        match self {
            Notif::Done => debug!("Notification: task done"),
            Notif::Log { log, .. } => debug!("Notification: log message - {}", log),
            Notif::Error {
                exit_code,
                guilty_node,
            } => {
                debug!(
                    "Notification: error (code {}, node {})",
                    exit_code, guilty_node
                )
            }
        }
    }
}
