use anyhow::Result;
use shiplift::{ContainerOptions, Docker, NetworkCreateOptions};

pub async fn create_network(name: &str) -> Result<()> {
    let docker = Docker::new();
    let networks = docker.networks();

    networks
        .create(&NetworkCreateOptions::builder(name).build())
        .await?;
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
