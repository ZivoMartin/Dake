use std::{net::SocketAddr, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{enc, makefile::RemoteMakefile, process_id::ProcessId};

pub trait MessageTrait: Clone + Serialize + Send {
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
    DaemonMessage,
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
pub struct Message<M: MessageTrait> {
    pub inner: M,
    pub pid: ProcessId,
    pub client: SocketAddr,
}

impl<M: MessageTrait> Message<M> {
    pub fn new(inner: M, pid: ProcessId, client: SocketAddr) -> Self {
        Self { inner, pid, client }
    }

    pub fn get_kind(&self) -> MessageKind {
        self.inner.get_kind()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DaemonMessage {
    NewProcess {
        makefiles: Vec<RemoteMakefile>,
        args: Vec<String>,
    },
    Distribute {
        makefile: RemoteMakefile,
    },
    Fetch {
        target: String,
        labeled_path: Option<PathBuf>,
    },
}

impl MessageTrait for DaemonMessage {
    fn get_kind(&self) -> MessageKind {
        MessageKind::DaemonMessage
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ProcessMessage {
    End,
}

impl MessageTrait for ProcessMessage {
    fn get_kind(&self) -> MessageKind {
        MessageKind::ProcessMessage
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum DistributerMessage {
    Ack,
    Failed,
}

impl MessageTrait for DistributerMessage {
    fn get_kind(&self) -> MessageKind {
        MessageKind::DistributerMessage
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum FetcherMessage {
    Object(Vec<u8>),
}

impl MessageTrait for FetcherMessage {
    fn get_kind(&self) -> MessageKind {
        MessageKind::FetcherMessage
    }
}
