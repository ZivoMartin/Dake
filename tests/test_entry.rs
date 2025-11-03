use std::path::PathBuf;

use anyhow::Result;
pub mod common;
mod test_basic;
mod test_fetch_chain;
mod test_redundant;

use crate::{
    common::cluster::{Cluster, clean_cluster, setup_cluster},
    test_basic::test_basic_build,
    test_fetch_chain::test_fetch_chain_build,
    test_redundant::test_redundant_build,
};

async fn run(
    cluster: &Cluster,
    (files, work_path, expected): (Vec<(PathBuf, String)>, PathBuf, String),
    caller: usize,
) -> Result<()> {
    cluster.push_files(files, &work_path).await?;

    cluster
        .start_dake(
            work_path.clone(),
            &cluster.nodes[caller],
            PathBuf::from(format!("caller_{caller}")),
        )
        .await?;

    cluster
        .confirm(
            &cluster.nodes[caller],
            "./main",
            Vec::new(),
            work_path,
            &expected,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn integration_suite() -> Result<()> {
    let cluster = setup_cluster().await?;

    let result = tokio::try_join!(
        run(cluster, test_basic_build(), 0),
        // run(cluster, test_fetch_chain_build(), 1),
        // run(cluster, test_redundant_build(), 0),
    );

    clean_cluster().await?;

    // Return the first error if any task failed
    result.map(|_| ())
}
