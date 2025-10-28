//! # Filesystem Utilities
//!
//! This module handles filesystem interactions for Dake.  
//!
//! Responsibilities include:
//! - Determining the persistent Dake working directory using `directories`.
//! - Initializing the filesystem structure on demand.
//! - Hashing process identifiers into unique filenames for storing remote makefiles.
//! - Writing remote makefiles received from other daemons.
//!
//! The design ensures that each `ProcessId` gets a unique hashed path, avoiding
//! collisions while remaining deterministic.

use anyhow::{Context, Result, bail};
use blake3::{self, Hash};
use directories::ProjectDirs;
use std::{
    env::var,
    fs::{create_dir, create_dir_all, read_dir, remove_dir_all, remove_file, write},
    path::PathBuf,
};
use tracing::{error, info, warn};

use crate::{env_variables::EnvVariable, makefile::RemoteMakefile, process_id::ProcessId};

/// Returns the base path for Dake's working directory.
///
/// The directory is chosen using the [`directories`] crate and follows
/// the convention:
/// `~/.local/share/dake` on Linux, or the platform equivalent.
///
/// # Errors
/// Fails if the project directory cannot be determined.
fn get_dake_path() -> Result<PathBuf> {
    var(EnvVariable::DakeSpacePath.to_string())
        .map(PathBuf::from)
        .or_else(|_| {
            ProjectDirs::from("com", "zivo_martin", "dake")
                .context("When fetching the project path.")
                .map(|d| d.project_path().to_path_buf())
        })
        .context("Failed to fetch dake space path.")
}

/// Initializes the Dake filesystem structure if not already present.

///
/// If the directory exists but is not a directory, this function fails.
///
/// # Returns
/// The path to the Dake working directory.
///
/// # Errors
/// Fails if directory creation is not possible.
pub fn init_fs() -> Result<PathBuf> {
    let path = get_dake_path()?;
    if path.exists() {
        if !path.is_dir() {
            error!("Dake path exists but is not a directory: {:?}", path);
            bail!("The path {path:?} exists but is not a directory.");
        }
        info!("Dake path already exists: {:?}", path);
    } else {
        create_dir_all(&path).context("Failed to create the dake directory")?;
        info!("Created Dake working directory at {:?}", path);
    }
    Ok(path)
}

/// Creates a hash from the [`ProcessId`], used to derive unique file paths.
///
/// The hash includes:
/// - The process socket IP
/// - The process working directory path
fn hash_socket_path(pid: &ProcessId) -> Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(pid.sock().to_string().as_bytes());
    hasher.update(pid.path().to_string_lossy().as_bytes());
    hasher.finalize()
}

/// Returns a unique path for storing a makefile associated with a [`ProcessId`].
///
/// Uses the first 16 bytes of the blake3 hash of the process identifier.
pub fn get_makefile_path(pid: &ProcessId) -> Result<PathBuf> {
    let hash = format!("{}", hash_socket_path(pid));
    let short = &hash.as_bytes()[..16];
    let mut path = init_fs()?;
    path.push(hex::encode(short));
    info!("Generated makefile path for pid {:?}: {:?}", pid, path);
    Ok(path)
}

/// Writes a [`RemoteMakefile`] to disk, associating it with the given [`ProcessId`].
///
/// The build folder is created if absent, and the Makefile is writed in build_folder/Makefile.
///
/// # Errors
/// Returns an error if writing to disk fails.
pub fn push_makefile(makefile: &RemoteMakefile, pid: &ProcessId) -> Result<()> {
    let mut path = get_makefile_path(pid)?;
    info!("Writing remote makefile for pid {:?} to {:?}", pid, path);

    if path.exists() {
        if path.is_file() {
            warn!("{path:?} is a file but is supposed to be a build folder.");
            remove_file(&path)?;
            info!(
                "Removing the file {path:?} and creating the building folder {path:?} for the process {pid:?}."
            );
            create_dir(&path)?;
        } else {
            info!("Build directory for {pid:?} was already created.")
        }
    } else {
        info!("Creating the building folder {path:?} for the process {pid:?}.");
        create_dir(&path)?;
    }
    info!("Creation of the build directory for {pid:?} has been a success.");

    path.push("Makefile");

    write(&path, makefile.makefile())
        .context("Failed to write the Makefile.")
        .map(|_| {
            info!(
                "Successfully wrote makefile for pid {:?} to {:?}",
                pid, path
            );
        })
}

/// Recursively deletes a file or directory and logs the total size removed.
pub fn clean() -> Result<()> {
    let path = get_dake_path()?;
    let size = calculate_size(&path)?;
    let _ = remove_dir_all(&path);
    info!("Removed {:?} ({} bytes)", path, size);
    Ok(())
}

/// Calculates the total size of a file or directory recursively.
fn calculate_size(path: &PathBuf) -> Result<u64> {
    if path.is_file() {
        Ok(std::fs::metadata(path)?.len())
    } else if path.is_dir() {
        let mut total = 0;
        for entry in read_dir(path)? {
            let entry = entry?;
            total += calculate_size(&entry.path())?;
        }
        Ok(total)
    } else {
        Ok(0)
    }
}
