mod message_ctx;
mod messages;
mod notif;
mod socket;
mod utils;

pub use self::{
    message_ctx::MessageCtx,
    messages::{
        AckMessage, DaemonMessage, FetcherMessage, Message, MessageHeader, MessageKind,
        MessageTrait, ProcessMessage,
    },
    notif::Notif,
    utils::{
        connect, contact_daemon_or_start_it, get_daemon_ip, get_daemon_port, get_daemon_sock,
        read_next_message, send_message, write_message,
    },
};

pub const DEFAULT_PORT: u16 = 1808;
pub const DAEMON_UNIX_SOCKET: &str = "/tmp/dake_daemon.sock";
