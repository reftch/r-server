#[cfg(test)]
mod tests;

use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

use crate::response::ContentType;

/// Gets an environment variable and parses it to type T.
/// Returns the default value if the variable is not set or cannot be parsed.
///
/// This function also handles boolean-like values such as "yes"/"no" and "1"/"0".
///
/// Example:
///
/// ```
/// use r_server::utils::get_env;
///
/// let int_value = get_env("value", 8080);
/// assert_eq!(int_value, 8080);
/// let bool_value = get_env("bool", true);
/// assert_eq!(bool_value, true);
/// let bool_value = get_env("bool", false);
/// assert_eq!(bool_value, false);
/// ```
pub fn get_env<T>(key: &str, default: T) -> T
where
    T: FromStr + Clone,
{
    let val = match env::var(key) {
        Ok(v) => v.trim().to_string(),
        Err(_) => return default,
    };

    // First, attempt to parse using the type's FromStr implementation (works for String, i32, f64, etc.)
    if let Ok(parsed) = val.parse::<T>() {
        return parsed;
    }

    // Second, handle common boolean patterns like "yes"/"no" or "1"/"0" by
    // converting them to "true"/"false" and attempting to parse again.
    let lower = val.to_lowercase();
    match lower.as_str() {
        "yes" | "1" | "true" => {
            if let Ok(parsed) = "true".parse::<T>() {
                return parsed;
            }
        }
        "no" | "0" | "false" => {
            if let Ok(parsed) = "false".parse::<T>() {
                return parsed;
            }
        }
        _ => {}
    }

    default
}

pub fn file_mtime_to_http_date(mtime: i64) -> String {
    if mtime < 0 {
        return String::new();
    }

    let timestamp = format_timestamp(mtime as u64, 0);

    // YYYY-MM-DD HH:MM:SS.mmm
    let year: i32 = timestamp[0..4].parse().unwrap_or(0);
    let month: usize = timestamp[5..7].parse().unwrap_or(0);
    let day = &timestamp[8..10];
    let hour = &timestamp[11..13];
    let min = &timestamp[14..16];
    let sec = &timestamp[17..19];

    if month == 0 || month > 12 {
        return String::new();
    }

    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    // Calculate weekday (1970-01-01 was Thursday)
    let days = mtime as u64 / 86400;
    let weekdays = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    let weekday = weekdays[((days + 4) % 7) as usize];

    format!(
        "{}, {} {} {:04} {}:{}:{} GMT",
        weekday,
        day,
        months[month - 1],
        year,
        hour,
        min,
        sec
    )
}

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

pub fn compute_etag(mtime: usize, size: usize) -> String {
    if mtime == 0 || size == 0 {
        return String::new();
    }

    format!("\"{:x}-{:x}\"", mtime, size)
}

pub fn get_file_info(
    path: &str,
    assets_path: &Path,
) -> Option<(Vec<u8>, ContentType, String, String)> {
    if path.is_empty() {
        return None;
    }

    let requested_path = Path::new(path);

    // Prevent directory traversal attacks
    if requested_path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return None;
    }

    let mut full_path =
        assets_path.join(requested_path.strip_prefix("/").unwrap_or(requested_path));

    if full_path.is_dir() {
        full_path.push("index.html");
    }

    if !full_path.is_file() {
        return None;
    }

    let file_metadata = fs::metadata(&full_path).ok()?;

    match fs::read(&full_path) {
        Ok(content) => {
            let content_type = ContentType::get_content_type(&full_path);

            let etag = compute_etag(
                file_metadata
                    .modified()
                    .ok()
                    .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
                    .map(|mtime| mtime.as_secs() as usize)
                    .unwrap_or(0),
                file_metadata.len() as usize,
            );

            let last_modified = file_metadata
                .modified()
                .ok()
                .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
                .map(|mtime| file_mtime_to_http_date(mtime.as_secs() as i64))
                .unwrap_or_default();

            Some((content, content_type, etag, last_modified))
        }

        Err(_) => None,
    }
}
