mod distribute;
mod fetch;
mod listen;
mod makefile;
mod messages;
mod new_process;
mod utils;

// NOTE: This is temporary and will be changed with a DNS system.
pub const DEFAULT_PORT: u16 = 1808;
pub const DEFAULT_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

use std::net::{IpAddr, Ipv4Addr};
pub use {
    distribute::distribute,
    fetch::fetch,
    listen::start,
    makefile::RemoteMakefile,
    messages::{DeamonMessage, MessageHeader, MessageKind, ProcessMessage},
    utils::{contact_deamon_or_start_it, get_deamon_address, read_next_message},
};
