use anyhow::{Context, Result, bail};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt::{Display, Formatter},
    hash::{Hash, Hasher},
    net::{IpAddr, SocketAddr as TcpSocketAddr},
    path::{Path, PathBuf},
    str::FromStr,
};
use tokio::net::unix::SocketAddr as UnixSocketAddr;
use tracing::{debug, error, warn};

/// Unified socket address abstraction supporting both TCP and Unix sockets.
#[derive(Clone, Debug)]
pub enum SocketAddr {
    Unix(UnixSocketAddr),
    Tcp(TcpSocketAddr),
}

impl SocketAddr {
    pub fn new_tcp(ip: IpAddr, port: u16) -> Self {
        Self::Tcp(TcpSocketAddr::new(ip, port))
    }

    pub fn new_unix(path: &Path) -> Result<Self> {
        let addr = std::os::unix::net::SocketAddr::from_pathname(path)
            .context("failed to create unix socket")?;
        Ok(Self::Unix(UnixSocketAddr::from(addr)))
    }

    pub fn ip(&self) -> Option<IpAddr> {
        match self {
            Self::Unix(_) => None,
            Self::Tcp(sock) => Some(sock.ip()),
        }
    }
}

impl FromStr for SocketAddr {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        if let Some(rest) = s.strip_prefix("unix:") {
            let path = Path::new(rest.trim());
            if !path.exists() {
                bail!("Unix socket path does not exist: {}", path.display());
            }
            Self::new_unix(&path)
        } else {
            let addr: TcpSocketAddr = s
                .parse()
                .with_context(|| format!("Invalid TCP socket address: {s}"))?;
            Ok(SocketAddr::Tcp(addr))
        }
    }
}

impl Display for SocketAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SocketAddr::Tcp(addr) => write!(f, "{addr}"),
            SocketAddr::Unix(addr) => {
                if let Some(path) = addr.as_pathname() {
                    write!(f, "unix:{}", path.display())
                } else {
                    // Abstract or unnamed Unix socket — uncommon but valid
                    warn!("Formatting unnamed Unix socket address");
                    write!(f, "unix:(unnamed)")
                }
            }
        }
    }
}

impl Serialize for SocketAddr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            SocketAddr::Unix(addr) => {
                if let Some(path) = addr.as_pathname() {
                    debug!("Serializing Unix socket address: {}", path.display());
                    serializer.serialize_newtype_variant("SocketAddr", 0, "Unix", &path)
                } else {
                    warn!("Serializing unnamed Unix socket address");
                    serializer.serialize_newtype_variant("SocketAddr", 0, "Unix", &"<unnamed>")
                }
            }
            SocketAddr::Tcp(addr) => {
                debug!("Serializing TCP socket address: {}", addr);
                serializer.serialize_newtype_variant("SocketAddr", 1, "Tcp", &addr)
            }
        }
    }
}

impl<'de> Deserialize<'de> for SocketAddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Unix(PathBuf),
            Tcp(TcpSocketAddr),
        }

        let helper = Repr::deserialize(deserializer).map_err(|e| {
            error!("Failed to deserialize SocketAddr: {}", e);
            e
        })?;

        Ok(match helper {
            Repr::Unix(path) => {
                debug!("Deserializing Unix socket from {:?}", path);
                Self::new_unix(&path).map_err(serde::de::Error::custom)?
            }
            Repr::Tcp(addr) => {
                debug!("Deserializing TCP socket from {}", addr);
                SocketAddr::Tcp(addr)
            }
        })
    }
}

impl From<TcpSocketAddr> for SocketAddr {
    fn from(value: TcpSocketAddr) -> Self {
        debug!("Converting from TcpSocketAddr to SocketAddr");
        Self::Tcp(value)
    }
}

impl From<UnixSocketAddr> for SocketAddr {
    fn from(value: UnixSocketAddr) -> Self {
        debug!("Converting from UnixSocketAddr to SocketAddr");
        Self::Unix(value)
    }
}

impl PartialEq for SocketAddr {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SocketAddr::Tcp(a), SocketAddr::Tcp(b)) => a == b,
            (SocketAddr::Unix(a), SocketAddr::Unix(b)) => {
                // UnixSocketAddr does not implement Eq directly, so we compare their paths.
                match (a.as_pathname(), b.as_pathname()) {
                    (Some(path_a), Some(path_b)) => path_a == path_b,
                    // Abstract or unnamed sockets can’t be compared by path, so fallback to raw bytes
                    (None, None) => a.is_unnamed() && b.is_unnamed(),
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

impl Eq for SocketAddr {}

impl Hash for SocketAddr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            SocketAddr::Tcp(addr) => {
                0u8.hash(state); // variant tag for disambiguation
                addr.hash(state);
            }
            SocketAddr::Unix(addr) => {
                1u8.hash(state);
                if let Some(path) = addr.as_pathname() {
                    path.hash(state);
                } else {
                    "<unnamed>".hash(state);
                }
            }
        }
    }
}
