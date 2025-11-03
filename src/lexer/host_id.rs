use anyhow::{Context, Error, Result};
use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    str::FromStr,
};

use crate::network::DEFAULT_PORT;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HostId {
    Socket(SocketAddr),
    Ip(IpAddr),
    Name(String),
}

impl HostId {
    pub fn resolve(self) -> Result<SocketAddr> {
        Ok(match self {
            HostId::Ip(ip) => SocketAddr::new(ip, DEFAULT_PORT),
            HostId::Socket(sock) => sock,
            HostId::Name(name) => (name, DEFAULT_PORT)
                .to_socket_addrs()?
                .next()
                .context("Failed to resolve DNS name {name}.")?,
        })
    }
}

impl FromStr for HostId {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s.parse::<SocketAddr>() {
            Ok(sock) => Self::Socket(sock),
            Err(_) => match s.parse::<IpAddr>() {
                Ok(ip) => Self::Ip(ip),
                Err(_) => HostId::Name(s.to_string()),
            },
        })
    }
}
