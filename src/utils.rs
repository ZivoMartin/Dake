use anyhow::{Result, bail};
use std::env::var;
use std::path::PathBuf;
use which::which;

use crate::env_variables::EnvVariable;

pub fn get_dake_path() -> Result<PathBuf> {
    let path_str = var(EnvVariable::BinaryPath.to_string()).unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            "target/debug/dake".to_string()
        } else {
            "dake".to_string()
        }
    });

    let path = PathBuf::from(&path_str);

    if path.exists() {
        if path.is_file() {
            return Ok(path);
        } else {
            bail!("DAKE_PATH does not point to a file: {}", path.display());
        }
    }

    // Otherwise, try to find it in PATH (only if it's a simple command like "dake")
    if let Ok(resolved) = which(&path_str) {
        return Ok(resolved);
    }

    bail!("Could not find '{}' in filesystem or PATH", path_str);
}
