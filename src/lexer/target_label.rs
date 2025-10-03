use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    str::FromStr,
};

use anyhow::{Error, Result};

use crate::network::DEFAULT_PORT;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TargetLabel {
    pub sock: SocketAddr,
    pub path: Option<PathBuf>,
}

impl TargetLabel {
    pub fn new(sock: SocketAddr, path: Option<PathBuf>) -> Self {
        Self { sock, path }
    }

    pub fn ip(&self) -> IpAddr {
        self.sock.ip()
    }
}

impl FromStr for TargetLabel {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let parse_sock = |sock: &str| -> Result<SocketAddr> {
            sock.parse::<SocketAddr>()
                .or_else(|_| Ok(SocketAddr::new(sock.parse()?, DEFAULT_PORT)))
        };
        Ok(match s.rsplit_once("|") {
            Some((sock, path)) => TargetLabel::new(parse_sock(sock)?, Some(path.parse()?)),
            None => TargetLabel::new(parse_sock(s)?, None),
        })
    }
}
