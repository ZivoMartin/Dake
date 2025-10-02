use std::{net::SocketAddr, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{enc, makefile::RemoteMakefile};

pub trait Message: Clone + Serialize + for<'a> Deserialize<'a> + Send {
    fn get_kind(&self) -> MessageKind;
}

#[derive(Serialize, Deserialize, Default)]
pub struct MessageHeader {
    pub size: u64,
    pub kind: MessageKind,
}

#[derive(Serialize, Deserialize, Default, Eq, PartialEq)]
pub enum MessageKind {
    #[default]
    DeamonMessage,
    ProcessMessage,
    DistributerMessage,
    FetcherMessage,
}

impl MessageHeader {
    pub fn new(size: u64, kind: MessageKind) -> Self {
        Self { size, kind }
    }

    pub fn get_header_length() -> usize {
        enc!(MessageHeader::default()).len()
    }

    pub fn wrap(mut msg: Vec<u8>, kind: MessageKind) -> Vec<u8> {
        let header = MessageHeader::new(msg.len() as u64, kind);
        let mut header = enc!(header);
        header.append(&mut msg);
        header
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DeamonMessage {
    NewProcess {
        makefiles: Vec<RemoteMakefile>,
        caller_addr: SocketAddr,
        entry_makefile_dir: PathBuf,
        args: Vec<String>,
    },
    Distribute(RemoteMakefile, PathBuf),
    Fetch {
        target: String,
        sock: SocketAddr,
    },
}

impl Message for DeamonMessage {
    fn get_kind(&self) -> MessageKind {
        MessageKind::DeamonMessage
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ProcessMessage {
    End,
}

impl Message for ProcessMessage {
    fn get_kind(&self) -> MessageKind {
        MessageKind::ProcessMessage
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DistributerMessage {
    Ack,
    Failed,
}

impl Message for DistributerMessage {
    fn get_kind(&self) -> MessageKind {
        MessageKind::DistributerMessage
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum FetcherMessage {
    Object(Vec<u8>),
}

impl Message for FetcherMessage {
    fn get_kind(&self) -> MessageKind {
        MessageKind::FetcherMessage
    }
}
