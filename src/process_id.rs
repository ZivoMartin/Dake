use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::network::DEFAULT_SOCK;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProcessId {
    pub sock: SocketAddr,
    pub path: PathBuf,
}

impl Default for ProcessId {
    fn default() -> Self {
        Self {
            sock: DEFAULT_SOCK,
            path: PathBuf::default(),
        }
    }
}

impl ProcessId {
    pub fn new(sock: SocketAddr, path: PathBuf) -> Self {
        Self { sock, path }
    }

    pub fn ip(&self) -> IpAddr {
        self.sock.ip()
    }
}
