use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, warn};

use crate::network::SocketAddr;

/// Represents the unique identifier of a process within a given project.
/// Combines a numeric process ID with a [`ProjectId`] that identifies the caller.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct ProcessId {
    pub id: u64,
    pub project_id: ProjectId,
}

impl ProcessId {
    /// Creates a process ID with the default numeric value (0).
    /// This is typically used for uninitialized or placeholder processes.
    pub fn new_default(project_id: ProjectId) -> Self {
        debug!(
            "Creating default ProcessId (id = 0) for project {:?}",
            project_id
        );
        Self { id: 0, project_id }
    }

    /// Creates a new [`ProcessId`] using an ID, socket address, and file path.
    pub fn new(id: u64, sock: SocketAddr, path: PathBuf) -> Self {
        debug!(
            "Creating ProcessId with id={} from socket {:?} and path {:?}",
            id, sock, path
        );
        Self {
            id,
            project_id: ProjectId::new(sock, path),
        }
    }

    /// Returns the socket address associated with this process.
    #[inline]
    pub fn sock(&self) -> SocketAddr {
        self.project_id.sock.clone()
    }

    /// Returns the numeric ID of this process.
    #[inline]
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the path from which this process originated.
    #[inline]
    pub fn path(&self) -> &PathBuf {
        &self.project_id.path
    }
}

/// Identifies a project by its caller’s socket address and working directory.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct ProjectId {
    /// The socket of the original caller.
    pub sock: SocketAddr,

    /// The working directory of the caller where `make` was executed.
    pub path: PathBuf,
}

impl ProjectId {
    /// Creates a new project identifier from a socket and file path.
    pub fn new(sock: SocketAddr, path: PathBuf) -> Self {
        debug!(
            "Creating ProjectId with socket {:?} and path {:?}",
            sock, path
        );

        // Warn if the provided path looks suspicious (e.g., empty or not absolute)
        if path.as_os_str().is_empty() {
            warn!("ProjectId created with an empty path: socket={:?}", sock);
        } else if !path.is_absolute() {
            warn!("ProjectId created with a non-absolute path: {:?}", path);
        }

        Self { sock, path }
    }
}
