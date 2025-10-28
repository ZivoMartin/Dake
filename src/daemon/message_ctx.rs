use crate::{daemon::state::State, network::Stream, process_id::ProcessId};

/// Context for handling a message, including current state and sender info.
pub struct MessageCtx<'a> {
    pub stream: &'a mut Stream,
    pub state: State,
    pub pid: ProcessId,
}

impl<'a> MessageCtx<'a> {
    /// Creates a new message context.
    pub fn new(stream: &'a mut Stream, state: State, pid: ProcessId) -> Self {
        Self { stream, state, pid }
    }
}
