use serde::{Deserialize, Serialize};

use crate::{network::SocketAddr, process_id::ProcessId};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProcessDatas {
    pub caller_daemon: SocketAddr,
    pub involved_hosts: Vec<SocketAddr>,
    pub args: Vec<String>,
    pub pid: ProcessId,
}

impl ProcessDatas {
    pub fn new(
        pid: ProcessId,
        caller_daemon: SocketAddr,
        involved_hosts: Vec<SocketAddr>,
        args: Vec<String>,
    ) -> Self {
        Self {
            involved_hosts,
            caller_daemon,
            args,
            pid,
        }
    }
}
