#[macro_export]
macro_rules! dec {
    ($bytes:expr) => {
        postcard::from_bytes(&$bytes[..])
    };
    ($bytes:expr, $t:ty) => {
        postcard::from_bytes::<$t>(&$bytes[..])
    };
}

#[macro_export]
macro_rules! enc {
    ($var:expr) => {
        postcard::to_allocvec(&$var)
    };
}

#[macro_export]
macro_rules! wrap {
    ($name:expr) => {
        std::sync::Arc::new(tokio::sync::Mutex::new($name))
    };
}
