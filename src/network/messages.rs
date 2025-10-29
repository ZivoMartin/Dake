//! # Messages Module
//!
//! This module defines the **message protocol** used in the Dake distributed
//! build system.  
//!
//! Messages are serialized with `postcard` and transmitted across TCP sockets
//! between the daemon, caller, distributor, and fetcher components.

use std::{fmt::Debug, path::PathBuf};

use anyhow::Result;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    daemon::ProcessDatas, enc, makefile::RemoteMakefile, network::SocketAddr, process_id::ProcessId,
};

/// A trait implemented by all message types.
///
/// Defines how to retrieve the corresponding [`MessageKind`].
pub trait MessageTrait: Clone + Serialize + Send + Debug {
    /// Returns the [`MessageKind`] associated with this message.
    fn get_kind(&self) -> MessageKind;
}

/// Header prepended to every serialized message.
///
/// Contains:
/// - `size`: Length of the serialized payload (in bytes)
/// - `kind`: The [`MessageKind`] of the message
#[derive(Default, Debug)]
pub struct MessageHeader {
    /// Size of the message payload in bytes.
    pub size: u64,

    /// The kind of the message (daemon, process, etc.).
    pub kind: MessageKind,
}

impl Serialize for MessageHeader {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut buf = [0u8; MessageHeader::SIZE];
        buf[..8].copy_from_slice(&self.size.to_le_bytes());
        buf[8] = self.kind as u8;
        serializer.serialize_bytes(&buf)
    }
}

impl<'de> Deserialize<'de> for MessageHeader {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: &[u8] = Deserialize::deserialize(deserializer)?;
        if bytes.len() != MessageHeader::SIZE {
            return Err(serde::de::Error::custom(format!(
                "invalid header length: expected {} bytes, got {}",
                MessageHeader::SIZE,
                bytes.len()
            )));
        }

        let size = u64::from_le_bytes(bytes[..8].try_into().unwrap());
        let kind = match bytes[8] {
            0 => MessageKind::DaemonMessage,
            1 => MessageKind::ProcessMessage,
            2 => MessageKind::AckMessage,
            3 => MessageKind::FetcherMessage,
            other => return Err(serde::de::Error::custom(format!("invalid kind: {}", other))),
        };

        Ok(Self { size, kind })
    }
}

/// Discriminator for all supported message categories.
#[repr(u8)]
#[derive(Serialize, Deserialize, Default, Eq, PartialEq, Debug, Copy, Clone)]
pub enum MessageKind {
    /// Message coming from or for the daemon.
    #[default]
    DaemonMessage,

    /// Message related to process lifecycle.
    ProcessMessage,

    /// Message used during distribution of makefiles.
    AckMessage,

    /// Message used by fetcher logic to transfer build objects.
    FetcherMessage,
}

static HEADER_LENGTH: OnceCell<usize> = OnceCell::new();

impl MessageHeader {
    const SIZE: usize = 9;

    /// Creates a new [`MessageHeader`].
    pub fn new(size: u64, kind: MessageKind) -> Self {
        Self { size, kind }
    }

    /// Returns the serialized length of a default message header.
    pub fn get_header_length() -> Result<usize> {
        HEADER_LENGTH
            .get_or_try_init(|| Ok(enc!(MessageHeader::default())?.len()))
            .map(|len| *len)
    }

    /// Prepends a serialized header to a message payload.
    ///
    /// # Arguments
    /// * `msg` - The serialized payload.
    /// * `kind` - The message kind for this payload.
    pub fn wrap(mut msg: Vec<u8>, kind: MessageKind) -> Result<Vec<u8>> {
        let header = MessageHeader::new(msg.len() as u64, kind);
        let mut header = enc!(header)?;
        header.append(&mut msg);
        Ok(header)
    }
}

/// A generic message wrapper sent across the network.
///
/// Each message contains:
/// - `inner`: The actual message payload (implements [`MessageTrait`])
/// - `pid`: The [`ProcessId`] of the sender
/// - `client`: The socket of the sender, if none then reuse the same stream
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Message<M: MessageTrait> {
    /// The actual message payload.
    pub inner: M,

    /// The process identifier of the sender.
    pub pid: ProcessId,
}

impl<M: MessageTrait> Message<M> {
    /// Constructs a new [`Message`] with the given payload, process, and client.
    pub fn new(inner: M, pid: ProcessId) -> Self {
        Self { inner, pid }
    }

    /// Returns the [`MessageKind`] of the contained payload.
    pub fn get_kind(&self) -> MessageKind {
        self.inner.get_kind()
    }
}

/// Messages exchanged with the daemon.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum DaemonMessage {
    /// Request for a fresh ProcessId for the given project.
    /// The process id contains a valid project id but a default process id.
    FreshId,

    /// Request to start a new process with given makefiles and arguments.
    NewProcess {
        /// Remote makefiles to distribute.
        makefiles: Vec<RemoteMakefile>,

        /// Arguments to forward to `make`.
        args: Vec<String>,
    },

    /// Request to distribute a single makefile to a remote host.
    NewMakefile {
        /// The remote makefile.
        makefile: RemoteMakefile,

        /// The process datas for the build process of the makefile.
        process_datas: ProcessDatas,
    },

    /// Request to fetch a target from a remote host.
    Fetch {
        /// The build target to fetch.
        target: String,

        /// An optional labeled path for fetching.
        labeled_path: Option<PathBuf>,
    },

    /// Submit a new log to forward to the caller on stdout
    StdoutLog { log: String },

    /// Submit a new log to forward to the caller on stderr
    StderrLog { log: String },

    /// Indicates that one of the make failed.
    MakeError {
        guilty_node: SocketAddr,
        exit_code: i32,
    },

    /// Indicate that the process is done
    Done,
}

impl MessageTrait for DaemonMessage {
    fn get_kind(&self) -> MessageKind {
        MessageKind::DaemonMessage
    }
}

/// Messages related to process lifecycle.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ProcessMessage {
    /// Response of the daemon, the pid of the message is the fresh pid.
    FreshId,
    /// Log form the remote make processes on stdout.
    StdoutLog { log: String },
    /// Log form the remote make processes on stderr.
    StderrLog { log: String },
    /// Indicates that the process has finished execution.
    End { exit_code: i32 },
}

impl MessageTrait for ProcessMessage {
    fn get_kind(&self) -> MessageKind {
        MessageKind::ProcessMessage
    }
}

/// Acknowledgment or failure messages
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum AckMessage {
    /// The makefile was successfully received.
    Ok,

    /// The makefile distribution failed.
    Failure,
}

impl MessageTrait for AckMessage {
    fn get_kind(&self) -> MessageKind {
        MessageKind::AckMessage
    }
}

/// Messages used by the fetcher to transfer objects or build artifacts.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum FetcherMessage {
    /// Encapsulates a build object (binary data).
    Object(Vec<u8>),
    /// Indicated that the fetch failed
    Failed,
}

impl MessageTrait for FetcherMessage {
    fn get_kind(&self) -> MessageKind {
        MessageKind::FetcherMessage
    }
}
