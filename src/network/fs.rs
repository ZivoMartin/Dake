use anyhow::{Context, Result, bail};
use blake3::{self, Hash};
use directories::ProjectDirs;
use std::{
    fs::{create_dir_all, write},
    path::PathBuf,
};

use crate::{makefile::RemoteMakefile, process_id::ProcessId};

fn get_dake_path() -> Result<PathBuf> {
    ProjectDirs::from("com", "zivo_martin", "dake")
        .context("When fetching the project path.")
        .map(|d| d.project_path().to_path_buf())
}

// As init is not heavy, we will re-init each time we want to interract with the dake directory.
pub fn init_fs() -> Result<PathBuf> {
    let path = get_dake_path()?;
    if path.exists() {
        if !path.is_dir() {
            bail!("The path {path:?} exists but is not a directory.");
        }
    } else {
        create_dir_all(&path).context("Failed to create the dake directory")?;
    }
    Ok(path)
}

fn hash_socket_path(pid: &ProcessId) -> Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(pid.sock.ip().to_string().as_bytes());
    hasher.update(pid.path.to_string_lossy().as_bytes());
    hasher.finalize()
}

fn get_makefile_path(pid: &ProcessId) -> Result<PathBuf> {
    let hash = format!("{}", hash_socket_path(pid));
    let short = &hash.as_bytes()[..16];
    let mut path = init_fs()?;
    path.push(hex::encode(short));
    Ok(path)
}

pub fn push_makefile(makefile: &RemoteMakefile, pid: &ProcessId) -> Result<()> {
    let path = get_makefile_path(pid)?;
    write(path, makefile.makefile()).context("Failed to write the Makefile.")
}
