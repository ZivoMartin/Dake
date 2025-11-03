use std::time::Duration;

pub const MUTEX_LOCK_TIMEOUT: Duration = Duration::from_secs(5);
pub const DAEMON_STARTUP_TIMEOUT: Duration = Duration::from_secs(3);
pub const DAEMON_RETRY_INTERVAL: Duration = Duration::from_millis(5);
pub const FETCH_FAILURE_DELAY: Duration = Duration::from_secs(90);
pub const DONE_NOTIFICATION_TIMEOUT: Duration = Duration::from_secs(3);

pub const CHUNK_SIZE: usize = 8 * 1024;
pub const INITIAL_PROCESS_ID: u64 = 1;
pub const EXIT_CODE_FAILURE: i32 = 1;
pub const CHANNEL_SIZE: usize = 100;
