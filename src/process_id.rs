//! # Process Identifier
//!
//! This module defines the [`ProcessId`] struct, which uniquely identifies a
//! process in the Dake distributed build system. A process is defined by:
//!
//! - The socket of the original caller
//! - The path on the caller machine where make has been called
//!
//! The `ProcessId` is attached to messages exchanged with the daemon so that
//! processes can be tracked across the distributed build network.

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

/// Uniquely identifies a process in the Dake distributed system.
///
/// A `ProcessId` contains:
/// - The process socket address (`sock`)
/// - The filesystem path (`path`) of the process
///
/// It is serializable to allow inclusion inside network messages.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct ProcessId {
    /// The socket of the original caller
    pub sock: SocketAddr,

    /// The path on the caller machine where make has been called
    pub path: PathBuf,
}

impl ProcessId {
    /// Creates a new `ProcessId` with the given socket and path.
    ///
    /// # Arguments
    /// * `sock` - The socket address of the process.
    /// * `path` - The working directory of the process.
    pub fn new(sock: SocketAddr, path: PathBuf) -> Self {
        Self { sock, path }
    }

    /// Returns the IP address of the process.
    pub fn ip(&self) -> IpAddr {
        self.sock.ip()
    }
}
