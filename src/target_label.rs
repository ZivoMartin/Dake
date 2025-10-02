use std::{
    fmt::{Display, Formatter},
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    str::FromStr,
};

use anyhow::{Error, Result};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TargetLabel {
    pub sock: SocketAddr,
    pub path: Option<PathBuf>,
}

impl Display for TargetLabel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.path {
            Some(p) => write!(f, "{}{}", self.sock, p.display()),
            None => write!(f, "{}", self.sock),
        }
    }
}

impl FromStr for TargetLabel {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s.split_once("|") {
            Some((socket, path)) => TargetLabel {
                sock: socket.parse()?,
                path: Some(path.parse()?),
            },
            _ => TargetLabel {
                sock: s.parse()?,
                path: None,
            },
        })
    }
}

impl TargetLabel {
    pub fn new(sock: SocketAddr, path: Option<PathBuf>) -> Self {
        Self { sock, path }
    }

    pub fn ip(&self) -> IpAddr {
        self.sock.ip()
    }
}
