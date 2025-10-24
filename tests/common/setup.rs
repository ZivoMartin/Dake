use crate::common::docker::{create_container, create_network};
use anyhow::Result;
use tokio::sync::OnceCell;

static CLUSTER: OnceCell<Vec<String>> = OnceCell::const_new();

pub async fn setup_cluster() -> Result<&'static Vec<String>> {
    CLUSTER
        .get_or_try_init(|| async {
            let network = "dake-net";
            create_network(network).await?;

            let mut nodes = Vec::new();
            for i in 0..3 {
                let name = format!("dake-node-{}", i);
                let id = create_container(&name, network, "dake-node").await?;
                nodes.push(id);
            }

            Ok(nodes)
        })
        .await
}
