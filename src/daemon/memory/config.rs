use std::{
    fs::{self, read_to_string},
    path::PathBuf,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tracing::{info, warn};

use crate::daemon::{DaemonId, fs::init_fs};

const CONFIG_NAME: &str = "config.json";

#[derive(Serialize, Deserialize, Clone, Copy, Default, Hash)]
pub struct DaemonConfig {
    os_pid: u32,
    id: DaemonId,
}

impl DaemonConfig {
    fn fresh() -> Self {
        Self {
            os_pid: std::process::id(),
            id: DaemonId::generate(),
        }
    }

    pub fn is_running() -> bool {
        info!("Checking weather the daemon is running or no.");
        match Self::load() {
            Ok(Some(config)) => {
                let mut sys = System::new_all();
                sys.refresh_processes(ProcessesToUpdate::All, true);
                sys.process(Pid::from(config.os_pid as usize)).is_some()
            }
            Ok(None) => false,
            Err(e) => {
                warn!("Failed to fetch config file: {e}");
                false
            }
        }
    }

    pub fn id(&self) -> DaemonId {
        self.id
    }

    fn path() -> Result<PathBuf> {
        let mut path = init_fs()?;
        path.push(CONFIG_NAME);
        Ok(path)
    }

    pub fn load_or_generate() -> Result<Self> {
        Self::load()?.map(Ok).unwrap_or_else(|| {
            let config = Self::fresh();
            config.save()?;
            Ok(config)
        })
    }

    fn load() -> Result<Option<Self>> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(None);
        }
        let data =
            read_to_string(&path).context(format!("Failed to read daemon state at {:?}", path))?;
        Ok(Some(serde_json::from_str(&data)?))
    }

    fn save(&self) -> Result<()> {
        let path = Self::path()?;
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)?)
            .context("Failed to write daemon state temp file")?;
        fs::rename(tmp, path).context("Failed to atomically replace daemon state file")
    }
}
