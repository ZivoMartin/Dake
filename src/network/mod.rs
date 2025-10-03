mod distribute;
mod fetch_handler;
mod fs;
mod listen;
mod makefile_receiver;
mod messages;
mod new_process;
mod utils;

pub const DEFAULT_PORT: u16 = 1808;
// Note: This will be changed in the future.
pub const DEFAULT_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
pub const DEFAULT_SOCK: SocketAddr = SocketAddr::new(DEFAULT_ADDR, DEFAULT_PORT);

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
pub use {
    distribute::distribute,
    listen::start,
    messages::{DaemonMessage, FetcherMessage, Message, MessageKind, ProcessMessage},
    utils::{contact_daemon_or_start_it, get_daemon_sock, read_next_message, send_message},
};
