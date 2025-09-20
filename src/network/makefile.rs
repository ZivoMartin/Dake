use std::{net::SocketAddr, path::PathBuf};

use anyhow::Result;
use derive_getters::Getters;
use serde::{Deserialize, Serialize};

use crate::network::utils::parse_raw_ip;

#[derive(Getters, Clone, Serialize, Deserialize)]
pub struct RemoteMakefile {
    makefile: String,
    ip: SocketAddr,
    path: PathBuf,
}

impl RemoteMakefile {
    pub fn new(makefile: String, ip: SocketAddr, path: PathBuf) -> Self {
        RemoteMakefile { makefile, ip, path }
    }

    pub fn cast_remote_makefile(
        m: dake::lexer::RemoteMakefile,
        path: PathBuf,
    ) -> Result<RemoteMakefile> {
        Ok(Self::new(m.makefile().clone(), parse_raw_ip(m.ip())?, path))
    }

    pub fn set_ip(&mut self, ip: SocketAddr) {
        self.ip = ip
    }
}
