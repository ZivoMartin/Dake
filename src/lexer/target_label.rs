//! # Target Label
//!
//! This module defines the [`TargetLabel`] struct, which represents the
//! destination of a build target in a distributed system.  
//!
//! A `TargetLabel` contains:
//! - A [`SocketAddr`] identifying the remote daemon (with optional default port).
//! - An optional [`PathBuf`] representing the build directory on that host.
//!
//! Parsing is provided via [`FromStr`], allowing convenient conversion from
//! string labels in Makefiles.

use std::{net::IpAddr, path::PathBuf, str::FromStr};

use anyhow::{Error, Result};
use tracing::info;

use crate::network::{DEFAULT_PORT, SocketAddr};

/// Represents a label for a build target in a distributed makefile.
///
/// Example formats:
/// - `"127.0.0.1:8080"` → `sock=127.0.0.1:8080, path=None`
/// - `"127.0.0.1|/tmp/build"` → `sock=127.0.0.1:DEFAULT_PORT, path=/tmp/build`
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TargetLabel {
    /// The socket (IP + port) of the remote daemon.
    pub sock: SocketAddr,
    /// Optional build directory associated with this target.
    pub path: Option<PathBuf>,
}

impl TargetLabel {
    /// Creates a new [`TargetLabel`] from a socket and optional path.
    pub fn new(sock: SocketAddr, path: Option<PathBuf>) -> Self {
        Self { sock, path }
    }

    pub fn ip(&self) -> Option<IpAddr> {
        self.sock.ip()
    }
}

impl FromStr for TargetLabel {
    type Err = Error;

    /// Parses a string into a [`TargetLabel`].
    ///
    /// # Supported formats
    /// - `"IP:PORT"` → uses provided port
    /// - `"IP"` → defaults to [`DEFAULT_PORT`]
    /// - `"IP:PORT|PATH"` → with optional build directory path and port
    /// - `"IP|PATH"` → with optional build directory path
    fn from_str(s: &str) -> Result<Self> {
        let parse_sock = |sock: &str| -> Result<SocketAddr> {
            sock.parse::<SocketAddr>().or_else(|_| {
                // If no port provided, fall back to DEFAULT_PORT
                Ok(SocketAddr::new_tcp(sock.parse()?, DEFAULT_PORT))
            })
        };

        Ok(match s.rsplit_once('|') {
            Some((sock, path)) => {
                println!("{sock} {path}");
                let addr = parse_sock(sock)?;

                let path_buf: PathBuf = path.parse()?;
                info!(
                    "TargetLabel: Parsed '{}' into addr={}, path={:?}",
                    s, addr, path_buf
                );
                TargetLabel::new(addr, Some(path_buf))
            }
            None => {
                println!("{s}");
                let addr = parse_sock(s)?;
                info!("TargetLabel: Parsed '{}' into addr={}, no path", s, addr);
                TargetLabel::new(addr, None)
            }
        })
    }
}
