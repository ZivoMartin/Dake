use crate::{daemon::handlers::OutputFile, network::SocketAddr};
use tracing::info;

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

    /// The target is unlock
    TargetUnlock { target: String },
}

impl Notif {
    /// Emits a trace log when a notification is sent or handled.
    pub fn trace(&self) {
        match self {
            Notif::Done => info!("Notification: task done"),
            Notif::Log { log, .. } => info!("Notification: log message - {}", log),
            Notif::Error {
                exit_code,
                guilty_node,
            } => {
                info!(
                    "Notification: error (code {}, node {})",
                    exit_code, guilty_node
                )
            }
            Notif::TargetUnlock { target } => info!("New target just unlocked: {target}"),
        }
    }
}
