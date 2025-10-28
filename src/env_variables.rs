use std::fmt::{Display, Formatter};

/// Represents environment variables used by the DAKE system.
/// Each variant corresponds to a specific environment variable name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvVariable {
    /// Port used by the daemon process
    DaemonPort,
    /// IP address of the daemon
    DaemonIp,
    /// Path to the binary executable of dake
    BinaryPath,
    /// Path to the DAKE workspace and data directory
    DakeSpacePath,
}

impl Display for EnvVariable {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            EnvVariable::DaemonPort => "DAKE_PORT",
            EnvVariable::DaemonIp => "DAKE_IP",
            EnvVariable::BinaryPath => "DAKE_PATH",
            EnvVariable::DakeSpacePath => "DAKE_SPACE_PATH",
        })
    }
}
