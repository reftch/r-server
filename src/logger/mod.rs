pub use self::logger::{print_log, LogLevel, set_level, format_timestamp, get_timestamp};
pub mod logger;

#[cfg(test)]
mod tests;
