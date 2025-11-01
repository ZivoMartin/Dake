use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf, str::FromStr};
use tracing::{info, warn};

use crate::daemon::DaemonId;

/// Represents the unique identifier of a process within a given project.
/// Combines a numeric process ID with a [`ProjectId`] that identifies the caller.
#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct ProcessId {
    pub id: u64,
    pub project_id: ProjectId,
}

impl ProcessId {
    /// Creates a process ID with the default numeric value (0).
    /// This is typically used for uninitialized or placeholder processes.
    pub fn process_less(project_id: ProjectId) -> Self {
        info!(
            "Creating default ProcessId (id = 0) for project {:?}",
            project_id
        );
        Self { id: 0, project_id }
    }

    pub fn is_process_less(&self) -> bool {
        self.id == 0
    }

    /// Creates a new [`ProcessId`] using an ID, daemon id, and file path.
    pub fn new(id: u64, daemon_id: DaemonId, path: PathBuf) -> Self {
        info!(
            "Creating ProcessId with id={} from daemon {} and path {:?}",
            id, daemon_id, path
        );
        Self {
            id,
            project_id: ProjectId::new(daemon_id, path),
        }
    }

    /// Returns the socket address associated with this process.
    #[inline]
    pub fn daemon_id(&self) -> DaemonId {
        self.project_id.daemon_id.clone()
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
#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct ProjectId {
    /// The daemon id of the original caller.
    pub daemon_id: DaemonId,

    /// The working directory of the caller where `make` was executed.
    pub path: PathBuf,
}

impl ProjectId {
    /// Creates a new project identifier from a socket and file path.
    pub fn new(daemon_id: DaemonId, path: PathBuf) -> Self {
        info!(
            "Creating ProjectId with id {:?} and path {:?}",
            daemon_id, path
        );

        // Warn if the provided path looks suspicious (e.g., empty or not absolute)
        if path.as_os_str().is_empty() {
            warn!(
                "ProjectId created with an empty path: daemon_id={:?}",
                daemon_id
            );
        } else if !path.is_absolute() {
            warn!("ProjectId created with a non-absolute path: {:?}", path);
        }

        Self { daemon_id, path }
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.daemon_id, self.path.display())
    }
}

impl FromStr for ProjectId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let (daemon_id, path_str) = s
            .split_once('-')
            .ok_or_else(|| anyhow!("invalid ProjectId format, expected '<daemon_id>-<path>'"))?;

        let daemon_id: DaemonId = daemon_id
            .parse()
            .map_err(|e| anyhow!("invalid daemon id in ProjectId: {e}"))?;

        let path = PathBuf::from(path_str);

        Ok(ProjectId { daemon_id, path })
    }
}

impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.id, self.project_id)
    }
}

impl FromStr for ProcessId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let (id_str, project_str) = s
            .split_once('@')
            .ok_or_else(|| anyhow!("invalid ProcessId format, expected '<id>@<project_id>'"))?;

        let id: u64 = id_str
            .parse()
            .map_err(|e| anyhow!("invalid process id: {e}"))?;

        let project_id: ProjectId = project_str.parse()?;

        Ok(ProcessId { id, project_id })
    }
}
