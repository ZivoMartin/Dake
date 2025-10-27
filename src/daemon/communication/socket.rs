use std::{
    net::SocketAddr as TcpSocketAddr, os::unix::net::SocketAddr as UnixSocketAddr, path::PathBuf,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug)]
pub enum SocketAddr {
    Unix(UnixSocketAddr),
    Tcp(TcpSocketAddr),
}

impl Serialize for SocketAddr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            SocketAddr::Unix(addr) => {
                // Convert UnixSocketAddr -> Option<PathBuf>
                if let Some(path) = addr.as_pathname() {
                    serializer.serialize_newtype_variant("SocketAddr", 0, "Unix", &path)
                } else {
                    // unnamed (abstract) socket
                    serializer.serialize_newtype_variant("SocketAddr", 0, "Unix", &"<unnamed>")
                }
            }
            SocketAddr::Tcp(addr) => {
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

        let helper = Repr::deserialize(deserializer)?;
        Ok(match helper {
            Repr::Unix(path) => {
                let addr =
                    UnixSocketAddr::from_pathname(&path).map_err(serde::de::Error::custom)?;
                SocketAddr::Unix(addr)
            }
            Repr::Tcp(addr) => SocketAddr::Tcp(addr),
        })
    }
}

impl From<TcpSocketAddr> for SocketAddr {
    fn from(value: TcpSocketAddr) -> Self {
        Self::Tcp(value)
    }
}

impl From<UnixSocketAddr> for SocketAddr {
    fn from(value: UnixSocketAddr) -> Self {
        Self::Unix(value)
    }
}
