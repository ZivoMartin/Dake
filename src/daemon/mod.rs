pub mod communication;
mod fs;
mod handlers;
mod listen;
mod operations;
mod process_datas;
mod state;

pub const DEFAULT_PORT: u16 = 1808;
// Note: This will be changed in the future.
pub const DEFAULT_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
pub const DEFAULT_SOCK: SocketAddr = SocketAddr::new(DEFAULT_ADDR, DEFAULT_PORT);

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
pub use {
    listen::start,
    operations::{broadcast_done, distribute, execute_make},
};
