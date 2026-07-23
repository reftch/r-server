pub use self::logger::{LogLevel, format_timestamp, get_timestamp, print_log, set_level};
pub mod logger;

#[cfg(test)]
mod tests;
