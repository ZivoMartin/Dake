use std::fmt::Display;

pub enum EnvVariable {
    DaemonPort,
    DaemonIp,
    BinaryPath,
    DakeSpacePath,
}

impl Display for EnvVariable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            EnvVariable::DaemonPort => "DAKE_PORT",
            EnvVariable::DaemonIp => "DAKE_IP",
            EnvVariable::BinaryPath => "DAKE_PATH",
            EnvVariable::DakeSpacePath => "DAKE_SPACE_PATH",
        })
    }
}
