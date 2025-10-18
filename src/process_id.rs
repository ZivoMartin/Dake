use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct ProcessId {
    pub id: u64,
    pub project_id: ProjectId,
}

impl ProcessId {
    pub fn new_default(project_id: ProjectId) -> Self {
        Self { id: 0, project_id }
    }

    pub fn new(id: u64, sock: SocketAddr, path: PathBuf) -> Self {
        Self {
            id,
            project_id: ProjectId::new(sock, path),
        }
    }

    pub fn sock(&self) -> SocketAddr {
        self.project_id.sock
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn path(&self) -> &PathBuf {
        &self.project_id.path
    }

    pub fn ip(&self) -> IpAddr {
        self.project_id.ip()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct ProjectId {
    /// The socket of the original caller
    pub sock: SocketAddr,

    /// The path on the caller machine where make has been called
    pub path: PathBuf,
}

impl ProjectId {
    pub fn new(sock: SocketAddr, path: PathBuf) -> Self {
        Self { sock, path }
    }

    pub fn ip(&self) -> IpAddr {
        self.sock.ip()
    }
}
