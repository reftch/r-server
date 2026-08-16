use std::fmt::Arguments;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU8, Ordering}; // Added for global state

use crate::utils::get_timestamp;

/// Represents the severity level of a log message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    /// Extremely fine-grained informational events that are most useful to debug an application.
    Trace = 0,
    /// Fine-grained informational events that are useful to debug an application.
    Debug = 1,
    /// Informational messages that highlight the progress of the application at coarse-grained level.
    Info = 2,
    /// Potentially harmful situations.
    Warn = 3,
    /// Error events that might still allow the application to continue running.
    Error = 4,
    /// Disables all logging.
    None = 5,
}

impl LogLevel {
    /// Returns the string representation of the log level.
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::None => "None",
        }
    }
}

// --- GLOBAL LOGGING STATE ---
/// The current global log level threshold.
static GLOBAL_LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

/// Sets the global minimum log level.
/// Only logs with this level or higher will be printed.
pub fn set_level(level: LogLevel) {
    GLOBAL_LOG_LEVEL.store(level as u8, Ordering::SeqCst);
}

/// Retrieves the current global log level threshold.
fn get_current_threshold() -> LogLevel {
    // Convert the stored u8 back into a LogLevel enum
    match GLOBAL_LOG_LEVEL.load(Ordering::SeqCst) {
        0 => LogLevel::Trace,
        1 => LogLevel::Debug,
        2 => LogLevel::Info,
        3 => LogLevel::Warn,
        4 => LogLevel::Error,
        5 => LogLevel::None,
        _ => LogLevel::Info, // Fallback
    }
}

/// Prints a log message to stdout if its level is greater than or equal to the current threshold.
pub fn print_log(level: LogLevel, module: &str, args: Arguments<'_>) {
    // IMPORTANT: Check the level FIRST before doing any work (like get_timestamp)
    if level < get_current_threshold() {
        return;
    }

    let now = get_timestamp();
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    let _ = writeln!(
        handle,
        "[{}] [{}] [{}] - {}",
        now,
        level.as_str(),
        module,
        args
    );
}

/// Logs a message at the `Trace` level.
#[macro_export]
macro_rules! trace {
    ($($arg:tt)+) => {
        $crate::logger::print_log($crate::logger::LogLevel::Trace, module_path!(), format_args!($($arg)+));
    };
}

/// Logs a message at the `Debug` level.
#[macro_export]
macro_rules! debug {
    ($($arg:tt)+) => {
        $crate::logger::print_log($crate::logger::LogLevel::Debug, module_path!(), format_args!($($arg)+));
    };
}

/// Logs a message at the `Info` level.
#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => {
        $crate::logger::print_log($crate::logger::LogLevel::Info, module_path!(), format_args!($($arg)+));
    };
}

/// Logs a message at the `Warn` level.
#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => {
        $crate::logger::print_log($crate::logger::LogLevel::Warn, module_path!(), format_args!($($arg)+));
    };
}

/// Logs a message at the `Error` level.
#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {
        $crate::logger::print_log($crate::logger::LogLevel::Error, module_path!(), format_args!($($arg)+));
    };
}


#[cfg(test)]
mod tests;
