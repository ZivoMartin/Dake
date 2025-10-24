mod common;
use common::setup::setup_cluster;
use anyhow::Result;

#[tokio::test(flavor = "multi_thread")]
async fn test_basic_build() -> Result<()> {
    let containers = setup_cluster().await?;

    // Pick one container and run your build
    let first = &containers[0];
    println!("Running build on container {first}");

    // Here you'd copy files and run your build
    // e.g. shiplift::ExecContainerOptions with "dake build"

    Ok(())
}

