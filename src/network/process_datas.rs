use std::net::SocketAddr;

use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Getters)]
pub struct ProcessDatas {
    /// Only the caller daemon has access to this information
    client_sock: Option<SocketAddr>,

    pub caller_daemon: SocketAddr,
    pub involved_hosts: Vec<SocketAddr>,
}

impl ProcessDatas {
    pub fn new_remote(caller_daemon: SocketAddr, involved_hosts: Vec<SocketAddr>) -> Self {
        Self {
            client_sock: None,
            involved_hosts,
            caller_daemon,
        }
    }

    pub fn new_local(
        caller_daemon: SocketAddr,
        client_sock: SocketAddr,
        involved_hosts: Vec<SocketAddr>,
    ) -> Self {
        Self {
            client_sock: Some(client_sock),
            involved_hosts,
            caller_daemon,
        }
    }

    pub fn is_local(&self) -> bool {
        self.client_sock.is_some()
    }

    pub fn is_remote(&self) -> bool {
        self.client_sock.is_none()
    }
}
