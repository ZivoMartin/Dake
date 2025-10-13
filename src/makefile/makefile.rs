use std::net::{IpAddr, SocketAddr};

use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Getters, Clone, Serialize, Deserialize, Debug)]
pub struct RemoteMakefile {
    makefile: String,
    sock: SocketAddr,
}

impl RemoteMakefile {
    pub fn new(makefile: String, sock: SocketAddr) -> Self {
        RemoteMakefile { makefile, sock }
    }

    pub fn ip(&self) -> IpAddr {
        self.sock.ip()
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
}
