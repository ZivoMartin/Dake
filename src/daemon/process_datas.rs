use serde::{Deserialize, Serialize};

use crate::network::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessDatas {
    pub caller_daemon: SocketAddr,
    pub involved_hosts: Vec<SocketAddr>,
    pub args: Vec<String>,
}

impl ProcessDatas {
    pub fn new(
        caller_daemon: SocketAddr,
        involved_hosts: Vec<SocketAddr>,
        args: Vec<String>,
    ) -> Self {
        Self {
            involved_hosts,
            caller_daemon,
            args,
        }
    }
}
