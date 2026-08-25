pub mod connection;
pub mod http;
pub mod https;
pub mod metadata;
pub(crate) mod shutdown;

const POLL_TIMEOUT: u64 = 100;
const STATIC_DIRECTORY: &str = "./templates";
