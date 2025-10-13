use std::net::SocketAddr;

use crate::{network::state::State, process_id::ProcessId};

#[derive(Clone)]
pub struct MessageCtx {
    pub state: State,
    pub pid: ProcessId,
    pub client: SocketAddr,
}

impl MessageCtx {
    pub fn new(state: State, pid: ProcessId, client: SocketAddr) -> Self {
        Self { state, pid, client }
    }
}
