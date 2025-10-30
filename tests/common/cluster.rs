use crate::common::docker::{
    container_exec, copy_into_container, create_container, create_network, get_container_ip,
    remove_container, remove_network,
};
use anyhow::{Context, Result};
use futures::future::try_join_all;
use tempfile::tempdir;

use std::{
    fs::{create_dir, remove_dir_all, remove_file, write},
    net::IpAddr,
    path::{Path, PathBuf},
};
use tokio::sync::OnceCell;
#[cfg(debug_assertions)]
use tracing::warn;

const LOG_DIR: &str = "logs";

#[derive(Debug)]
pub struct Cluster {
    pub network: String,
    pub nodes: Vec<String>,
    pub nodes_ips: Vec<IpAddr>,
}

impl Cluster {
    /// Create and start all containers for the cluster.
    pub async fn setup(size: usize) -> Result<Self> {
        let network = "dake-net".to_string();
        println!("Creating network {network}");
        create_network(&network).await?;

        let log_dir = PathBuf::from(LOG_DIR);
        if log_dir.is_dir() {
            remove_dir_all(&log_dir).context("Failed to clean logs dir.")?;
        } else if log_dir.is_file() {
            remove_file(&log_dir).context("Failed to remove logs file.")?;
        }
        create_dir(log_dir)?;

        let mut nodes = Vec::new();
        for i in 0..size {
            let name = format!("dake-node-{}", i);
            println!("Creating container {name}");
            let id = create_container(&name, &network, "dake-node").await?;
            nodes.push(id);
        }

        let nodes_ips = try_join_all(nodes.iter().map(|id| async {
            get_container_ip(id, &network).await.and_then(|ip| {
                println!("Got the ip {ip} from docker.");
                ip.parse::<IpAddr>()
                    .context(format!("Failed to parse {ip} into an ip address."))
            })
        }))
        .await?;

        for node in &nodes {
            container_exec(
                node,
                "dake",
                vec!["daemon"],
                PathBuf::from("/"),
                Some(PathBuf::from(format!("{LOG_DIR}/log_daemon_{node}"))),
                true,
            )
            .await?;
        }

        Ok(Self {
            network,
            nodes,
            nodes_ips,
        })
    }

    pub async fn clean(&self) -> Result<()> {
        println!("Cleaning up cluster ({} containers)", self.nodes.len());

        // Remove containers first
        for id in &self.nodes {
            match remove_container(id).await {
                Ok(_) => println!("Removed container {id}"),
                Err(e) => warn!("Failed to remove container {id}: {e}"),
            }
        }

        // Then remove the network
        match remove_network(&self.network).await {
            Ok(_) => println!("Removed network {}", self.network),
            Err(e) => warn!("Failed to remove network {}: {e}", self.network),
        }

        Ok(())
    }

    pub async fn push_files(&self, files: Vec<(PathBuf, String)>, dest_dir: &Path) -> Result<()> {
        let tmp_dir = tempdir()?;

        for (path, mut content) in files {
            let path = tmp_dir.path().join(path);
            for i in 0..self.nodes.len() {
                let ip = &self.nodes_ips[i].to_string();
                println!("Replacing NODE-{i} with {ip}");
                content = content.replace(&format!("NODE-{i}"), ip)
            }
            println!("{content}");
            write(path.clone(), content)
                .context(format!("Failed to write into {}.", path.display()))?
        }

        for node in &self.nodes {
            println!("Injecting project files into {node}");
            copy_into_container(node, tmp_dir.path(), dest_dir).await?;
        }
        Ok(())
    }

    pub async fn start_dake(&self, dest_path: PathBuf, id: &str, output: PathBuf) -> Result<()> {
        let mut path = PathBuf::from(LOG_DIR);
        path.push(&output);

        container_exec(id, "dake", vec![], dest_path, Some(path), false)
            .await
            .context(format!("Failed to execute dake on {id}"))?;
        Ok(())
    }
}

static CLUSTER: OnceCell<Cluster> = OnceCell::const_new();

/// Set up the global cluster (only once per test session).
pub async fn setup_cluster() -> Result<&'static Cluster> {
    CLUSTER
        .get_or_try_init(|| async {
            println!("Setting up global cluster...");
            Cluster::setup(3).await
        })
        .await
}

/// Clean up all containers created by the global cluster.
pub async fn clean_cluster() -> Result<()> {
    if let Some(cluster) = CLUSTER.get() {
        println!("Cleaning global cluster...");
        cluster.clean().await?;
    } else {
        warn!("Global cluster was never initialized");
    }
    Ok(())
}
