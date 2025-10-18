use std::{net::IpAddr, time::Duration};

use anyhow::{Context, Result};
use tokio::{
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tracing::{info, warn};

pub async fn accept_specific_connection(
    listener: &TcpListener,
    expected_ip: IpAddr,
) -> Result<TcpStream> {
    let timer = timeout(Duration::from_secs(10), async move {
        loop {
            match listener.accept().await {
                Ok((tcp_stream, addr)) => {
                    let ip = addr.ip();
                    if expected_ip == ip {
                        info!("Accepted connection from {expected_ip}.");
                        break tcp_stream;
                    } else {
                        warn!("Unexpected connection received: expected {expected_ip}, got {ip}.");
                    }
                }
                Err(e) => {
                    warn!("The listener failed to accept a connection: {e}");
                }
            }
        }
    });
    timer
        .await
        .context("Failed to wait for the connection from {expected_ip}.")
}
