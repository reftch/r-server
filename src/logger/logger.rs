use std::fmt::Arguments;
use std::io::{self, Write};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::time::{SystemTime, UNIX_EPOCH};

// --- LOG LEVELS ---
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    None = 5,
}

impl LogLevel {
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

// --- INTERNAL DATA STRUCTURE ---
struct LogEntry {
    level: LogLevel,
    module: String,
    content: String,
    timestamp: String,
}

// --- GLOBAL STATE ---
static GLOBAL_LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

/// This is the "Magic" part.
/// It holds our sender and initializes the background thread on its first use.
static LOG_SENDER: OnceLock<Sender<LogEntry>> = OnceLock::new();

/// Sets the global log level threshold. Only messages with a level
/// equal to or higher than this will be logged.
///
/// # Examples
///
/// ```rust
/// use r_server::logger::{set_level, LogLevel};
/// set_level(LogLevel::Warn);
/// ```
pub fn set_level(level: LogLevel) {
    GLOBAL_LOG_LEVEL.store(level as u8, Ordering::SeqCst);
}

fn get_current_threshold() -> LogLevel {
    match GLOBAL_LOG_LEVEL.load(Ordering::SeqCst) {
        0 => LogLevel::Trace,
        1 => LogLevel::Debug,
        2 => LogLevel::Info,
        3 => LogLevel::Warn,
        4 => LogLevel::Error,
        5 => LogLevel::None,
        _ => LogLevel::Info,
    }
}

/// This function handles the background thread lifecycle.
/// It's called automatically by print_log via the OnceLock.
fn get_sender() -> &'static Sender<LogEntry> {
    LOG_SENDER.get_or_init(|| {
        let (tx, rx) = channel::<LogEntry>();

        // Spawn the background worker thread
        std::thread::spawn(move || {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            for entry in rx {
                let _ = writeln!(
                    handle,
                    "[{}] [{}] [{}] - {}",
                    entry.timestamp,
                    entry.level.as_str(),
                    entry.module,
                    entry.content
                );
                // Flush ensures the message appears in the terminal immediately
                let _ = handle.flush();
            }
        });

        tx
    })
}

/// Formats a duration into a timestamp string: YYYY-MM-DD HH:MM:SS.mmm
///
/// # Examples
///
/// ```rust
/// use r_server::logger::format_timestamp;
/// let ts = format_timestamp(0, 500);
/// assert_eq!(ts, "1970-01-01 00:00:00.500");
/// ```
pub fn format_timestamp(total_seconds: u64, millis: u64) -> String {
    let sec = total_seconds % 60;
    let min = (total_seconds / 60) % 60;
    let hour = (total_seconds / 3600) % 24;
    let mut days = total_seconds / 86400;

    let mut year = 1970;
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let month_days = [
        31,
        if is_leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for m_days in month_days.iter() {
        if days < *m_days {
            break;
        }
        days -= m_days;
        month += 1;
    }
    let month = if month > 12 { 12 } else { month };

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        year,
        month,
        days + 1,
        hour,
        min,
        sec,
        millis
    )
}

pub fn get_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    format_timestamp(now.as_secs(), now.as_millis() as u64 % 1000)
}

/// Prints a log message to stdout via the background thread.
///
/// # Examples
///
/// ```rust
/// use r_server::logger::{print_log, LogLevel};
/// print_log(LogLevel::Info, "test_module", format_args!("Hello, world!"));
/// ```
pub fn print_log(level: LogLevel, module: &str, args: Arguments<'_>) {
    if level < get_current_threshold() {
        return;
    }

    let now = get_timestamp();
    let content = format!("{}", args); // Format here so it's thread-safe

    let entry = LogEntry {
        level,
        module: module.to_string(),
        content,
        timestamp: now,
    };

    // We fetch the sender (which initializes the thread on the first call)
    // and send the message immediately without blocking main execution.
    get_sender().send(entry).ok();
}

// --- MACROS ---
#[macro_export]
macro_rules! trace {
    ($($arg:tt)+) => {
        $crate::logger::print_log($crate::logger::LogLevel::Trace, module_path!(), format_args!($($arg)+));
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)+) => {
        $crate::logger::print_log($crate::logger::LogLevel::Debug, module_path!(), format_args!($($arg)+));
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => {
        $crate::logger::print_log($crate::logger::LogLevel::Info, module_path!(), format_args!($($arg)+));
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => {
        $crate::logger::print_log($crate::logger::LogLevel::Warn, module_path!(), format_args!($($arg)+));
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {
        $crate::logger::print_log($crate::logger::LogLevel::Error, module_path!(), format_args!($($arg)+));
    };
}
