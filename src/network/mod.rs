mod broadcast;
mod messages;
mod socket;
mod stream;
mod utils;

pub use self::{
    broadcast::{broadcast_message, broadcast_messages},
    messages::{
        AckMessage, DaemonMessage, FetcherMessage, Message, MessageHeader, MessageKind,
        MessageTrait, ProcessMessage,
    },
    socket::SocketAddr,
    stream::Stream,
    utils::{
        connect, connect_with_daemon_or_start_it, get_daemon_ip, get_daemon_port,
        get_daemon_tcp_sock, get_daemon_unix_sock, read_next_message, send_message, write_message,
    },
};

pub const DEFAULT_PORT: u16 = 1808;
pub const DAEMON_UNIX_SOCKET: &str = "/tmp/dake_daemon.sock";
