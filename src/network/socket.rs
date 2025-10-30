use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Formatter},
    hash::Hash,
    net::{IpAddr, SocketAddr as TcpSocketAddr},
    path::{Path, PathBuf},
    str::FromStr,
};
use tokio::net::unix::SocketAddr as UnixSocketAddr;

const UNNAMED_UNIX: &str = "unix:unnamed";

/// Unified socket address abstraction supporting both TCP and Unix sockets.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SocketAddr {
    Unix(Option<PathBuf>),
    Tcp(TcpSocketAddr),
}

impl SocketAddr {
    pub fn new_tcp(ip: IpAddr, port: u16) -> Self {
        Self::Tcp(TcpSocketAddr::new(ip, port))
    }

    pub fn new_unix(path: PathBuf) -> Result<Self> {
        Ok(Self::Unix(Some(path)))
    }

    pub fn new_unnamed_unix() -> Result<Self> {
        Ok(Self::Unix(None))
    }

    pub fn ip(&self) -> Option<IpAddr> {
        self.get_tcp().map(|sock| sock.ip())
    }

    pub fn get_tcp(&self) -> Option<TcpSocketAddr> {
        match self {
            Self::Unix(_) => None,
            Self::Tcp(sock) => Some(*sock),
        }
    }

    pub fn get_unix(&self) -> Option<Option<PathBuf>> {
        match self {
            Self::Unix(sock) => Some(sock.clone()),
            Self::Tcp(_) => None,
        }
    }

    pub fn is_unix(&self) -> bool {
        matches!(self, Self::Unix(_))
    }

    pub fn is_tcp(&self) -> bool {
        matches!(self, Self::Tcp(_))
    }
}

impl Default for SocketAddr {
    fn default() -> Self {
        Self::Unix(None)
    }
}

impl FromStr for SocketAddr {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s.parse::<TcpSocketAddr>() {
            Ok(addr) => Self::Tcp(addr),
            Err(_) => {
                if s == UNNAMED_UNIX {
                    Self::Unix(None)
                } else {
                    Self::Unix(Some(s.parse::<PathBuf>()?))
                }
            }
        })
    }
}

impl Display for SocketAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SocketAddr::Tcp(addr) => addr.fmt(f),
            SocketAddr::Unix(addr) => match addr {
                Some(path) => write!(f, "{}", path.display()),
                None => UNNAMED_UNIX.fmt(f),
            },
        }
    }
}

impl From<&Path> for SocketAddr {
    fn from(value: &Path) -> Self {
        Self::Unix(Some(PathBuf::from(value)))
    }
}

impl From<TcpSocketAddr> for SocketAddr {
    fn from(value: TcpSocketAddr) -> Self {
        Self::Tcp(value)
    }
}

impl From<UnixSocketAddr> for SocketAddr {
    fn from(value: UnixSocketAddr) -> Self {
        Self::Unix(value.as_pathname().map(PathBuf::from))
    }
}
