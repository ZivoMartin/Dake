#[macro_export]
macro_rules! dec {
    ($bytes:expr $(,$t:ty)?) => {{ bincode::deserialize(&($bytes[..])) }};
}

#[macro_export]
macro_rules! enc {
    ($var:expr) => {
        bincode::serialize(&$var)
    };
}

#[macro_export]
macro_rules! wrap {
    ($name:expr) => {
        std::sync::Arc::new(tokio::sync::Mutex::new($name))
    };
}
