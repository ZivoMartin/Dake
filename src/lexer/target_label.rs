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

use std::{path::PathBuf, str::FromStr};

use anyhow::{Error, Result};
use tracing::info;

use crate::lexer::HostId;

/// Represents a label for a build target in a distributed makefile.
///
/// Example formats:
/// - `"127.0.0.1:8080"` → `sock=127.0.0.1:8080, path=None`
/// - `"127.0.0.1|/tmp/build"` → `sock=127.0.0.1:DEFAULT_PORT, path=/tmp/build`
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TargetLabel {
    /// The socket (IP + port) of the remote daemon.
    pub id: HostId,
    /// Optional build directory associated with this target.
    pub path: Option<PathBuf>,
}

impl TargetLabel {
    /// Creates a new [`TargetLabel`] from a socket and optional path.
    pub fn new(id: HostId, path: Option<PathBuf>) -> Self {
        Self { id, path }
    }
}

impl FromStr for TargetLabel {
    type Err = Error;

    /// Parses a string into a [`TargetLabel`].
    ///
    /// # Supported formats
    /// - `"IP:PORT"` -> uses provided port
    /// - `"IP"` -> defaults to [`DEFAULT_PORT`]
    /// - `"IP:PORT|PATH"` -> with optional build directory path and port
    /// - `"IP|PATH"` -> with optional build directory path
    fn from_str(s: &str) -> Result<Self> {
        Ok(match s.rsplit_once('|') {
            Some((sock, path)) => {
                let id = sock.parse::<HostId>()?;

                let path_buf: PathBuf = path.parse()?;
                info!(
                    "TargetLabel: Parsed '{}' into id={:?}, path={:?}",
                    s, id, path_buf
                );
                TargetLabel::new(id, Some(path_buf))
            }
            None => {
                let id = s.parse::<HostId>()?;
                info!("TargetLabel: Parsed '{}' into addr={:?}, no path", s, id);
                TargetLabel::new(id, None)
            }
        })
    }
}
