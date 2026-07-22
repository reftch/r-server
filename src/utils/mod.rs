#[cfg(test)]
mod tests;

use std::env;
use std::str::FromStr;

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
