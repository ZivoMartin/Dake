use std::{
    fs::File,
    io::Write,
    iter::once,
    path::{Path, PathBuf},
};

use anyhow::{Context, Error, Result};
use futures::StreamExt;
use shiplift::{
    ContainerOptions, Docker, ExecContainerOptions, NetworkCreateOptions, tty::TtyChunk,
};
use tokio::{process::Command, spawn};

pub async fn create_network(name: &str) -> Result<()> {
    let docker = Docker::new();
    let networks = docker.networks();

    networks
        .create(&NetworkCreateOptions::builder(name).build())
        .await?;
    Ok(())
}

pub async fn remove_network(name: &str) -> Result<()> {
    let docker = Docker::new();
    let networks = docker.networks();
    networks.get(name).delete().await?;
    Ok(())
}

pub async fn create_container(name: &str, network: &str, image: &str) -> Result<String> {
    let docker = Docker::new();
    let container = docker
        .containers()
        .create(
            &ContainerOptions::builder(image)
                .name(name)
                .network_mode(network)
                .cmd(vec!["dake", "daemon"])
                .build(),
        )
        .await?;

    docker.containers().get(&container.id).start().await?;
    Ok(container.id)
}

pub async fn remove_container(id: &str) -> Result<()> {
    let docker = Docker::new();
    docker
        .containers()
        .get(id)
        .remove(shiplift::RmContainerOptions::builder().force(true).build())
        .await?;
    Ok(())
}

pub async fn get_container_ip(container_id: &str, network: &str) -> Result<String> {
    let docker = Docker::new();
    let container = docker.containers().get(container_id);
    let info = container
        .inspect()
        .await
        .context("Failed to inspect container")?;

    let network_info = info
        .network_settings
        .networks
        .get(network)
        .context(format!("Network '{}' not found on container", network))?;

    let ip = network_info.ip_address.clone();

    assert!(!ip.is_empty(), "Failed to fetch the ip of {container_id}");

    Ok(ip.clone())
}

/// Copy files into a container under `/project`
pub async fn copy_into_container(container: &str, src_dir: &Path, dest_dir: &Path) -> Result<()> {
    let status = Command::new("docker")
        .args([
            "cp",
            src_dir
                .to_str()
                .context("Source directory is not displayable.")?,
            &format!(
                "{container}:{}",
                dest_dir
                    .to_str()
                    .context("Destination directory is not displayable.")?
            ),
        ])
        .status()
        .await
        .context("Failed to copy project files to container")?;
    assert!(status.success(), "docker cp failed");
    Ok(())
}

pub async fn container_exec(
    id: &str,
    command: &str,
    args: Vec<&str>,
    output: Option<PathBuf>,
    background: bool,
) -> Result<()> {
    let handler = {
        let id = id.to_string();
        let command = command.to_string();
        let args = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        async move {
            let docker = Docker::new();
            let container = docker.containers().get(&id);
            let mut stream = container.exec(
                &ExecContainerOptions::builder()
                    .cmd(
                        once(command.as_str())
                            .chain(args.iter().map(|s| s.as_str()))
                            .collect(),
                    )
                    .attach_stdout(true)
                    .attach_stderr(true)
                    .build(),
            );

            if let Some(path) = output {
                let mut f = File::create(path).context("Failed to open the log file.")?;
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(TtyChunk::StdOut(bytes)) | Ok(TtyChunk::StdErr(bytes)) => {
                            f.write_all(&bytes)?;
                        }
                        Ok(TtyChunk::StdIn(_)) => {}
                        Err(e) => eprintln!("Stream error: {}", e),
                    }
                }
            } else {
                while let Some(chunk) = stream.next().await {
                    if let Ok(TtyChunk::StdOut(bytes)) | Ok(TtyChunk::StdErr(bytes)) = chunk {
                        print!("{}", String::from_utf8_lossy(&bytes));
                    }
                }
            }

            Ok::<(), Error>(())
        }
    };

    if background {
        spawn(handler);
    } else {
        handler.await?;
    }

    Ok(())
}
