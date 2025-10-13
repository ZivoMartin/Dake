mod fs;
mod handlers;
mod listen;
mod message_ctx;
mod messages;
mod process_datas;
mod state;
mod utils;

pub const DEFAULT_PORT: u16 = 1808;
// Note: This will be changed in the future.
pub const DEFAULT_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
pub const DEFAULT_SOCK: SocketAddr = SocketAddr::new(DEFAULT_ADDR, DEFAULT_PORT);

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
pub use {
    handlers::distribute,
    listen::start,
    messages::{DaemonMessage, FetcherMessage, Message, MessageKind, ProcessMessage},
    utils::{contact_daemon_or_start_it, get_daemon_sock, read_next_message, send_message},
};
