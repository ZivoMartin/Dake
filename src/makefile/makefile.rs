use std::net::IpAddr;

use derive_getters::Getters;
use serde::{Deserialize, Serialize};

use crate::network::SocketAddr;

#[derive(Getters, Clone, Serialize, Deserialize, Debug)]
pub struct RemoteMakefile {
    makefile: String,
    sock: SocketAddr,
}

impl RemoteMakefile {
    pub fn new(makefile: String, sock: SocketAddr) -> Self {
        RemoteMakefile { makefile, sock }
    }

    pub fn set_sock(&mut self, sock: SocketAddr) {
        self.sock = sock
    }

    pub fn push_content(&mut self, content: &str) {
        self.makefile.push_str(content);
    }

    pub fn drop_makefile(self) -> String {
        self.makefile
    }

    pub fn ip(&self) -> Option<IpAddr> {
        self.sock.ip()
    }
}
