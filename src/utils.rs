use anyhow::{Result, bail};
use std::path::PathBuf;
use std::{env::var, path::Path};
use tracing::{error, info};
use which::which;

use crate::env_variables::EnvVariable;

/// Attempts to locate the DAKE binary on the system.
/// Returns the absolute path to the binary if found, or an error otherwise.
pub fn get_dake_path() -> Result<PathBuf> {
    info!("Attempting to resolve DAKE binary path...");

    // Retrieve environment variable or fall back to defaults
    let path_str = var(EnvVariable::BinaryPath.to_string()).unwrap_or_else(|_| {
        let p = format!(
            "target/{}/dake",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        );
        Path::new(&p)
            .exists()
            .then(|| p)
            .unwrap_or_else(|| "dake".to_string())
    });

    info!("Raw path string resolved to '{}'", path_str);

    let path = PathBuf::from(&path_str);

    // Case 1: The path exists in the filesystem
    if path.exists() {
        info!("Found existing path: {:?}", path);

        if path.is_file() {
            info!("Resolved DAKE binary path: {}", path.display());
            return Ok(path);
        } else {
            error!("DAKE_PATH does not point to a file: {}", path.display());
            bail!(
                "DAKE_PATH does not point to a valid file: {}",
                path.display()
            );
        }
    }

    // Case 2: Try to find it in PATH (only relevant for short commands like "dake")
    info!("Path does not exist, attempting lookup via system PATH...");
    if let Ok(resolved) = which(&path_str) {
        info!("Found DAKE binary in PATH at: {}", resolved.display());
        return Ok(resolved);
    }

    // Case 3: Not found anywhere — fatal error
    error!("Could not find '{}' in filesystem or PATH", path_str);
    bail!("Could not find '{}' in filesystem or PATH", path_str);
}
