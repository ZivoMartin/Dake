#[macro_export]
macro_rules! dec {
    ($bytes:expr $(,$t:ty)?) => {{ bincode::deserialize(&($bytes[..])) }};
}

#[macro_export]
macro_rules! enc {
    ($var:expr) => {
        bincode::serialize(&$var).unwrap()
    };
    ($var:expr, $bytes:expr) => {
        match bincode::serialize(&$var) {
            Ok(mut serialized) => $bytes.append(&mut serialized),
            Err(e) => panic!("Serialization error: {:?}", e),
        }
    };
}

#[macro_export]
macro_rules! wrap {
    ($name:expr) => {
        std::sync::Arc::new(tokio::sync::Mutex::new($name))
    };
}
