use super::*;

#[test]
fn test_log_level_as_str() {
    assert_eq!(LogLevel::Debug.as_str(), "DEBUG");
    assert_eq!(LogLevel::Info.as_str(), "INFO");
    assert_eq!(LogLevel::Warn.as_str(), "WARN");
    assert_eq!(LogLevel::Error.as_str(), "ERROR");
}

#[test]
fn test_log_level_ordering() {
    assert!(LogLevel::Debug < LogLevel::Info);
    assert!(LogLevel::Info < LogLevel::Warn);
    assert!(LogLevel::Warn < LogLevel::Error);
}

#[test]
fn test_format_timestamp_with_millis() {
    let result = format_timestamp(0, 123);

    assert_eq!(result, "1970-01-01 00:00:00.123");
}

#[test]
fn test_format_timestamp_known_date() {
    // 2024-01-01 00:00:00 UTC
    // Unix timestamp = 1704067200
    let result = format_timestamp(1704067200, 999);

    assert_eq!(result, "2024-01-01 00:00:00.999");
}

#[test]
fn test_format_timestamp_leap_year() {
    // 2024-02-29 12:30:45
    let result = format_timestamp(1709213445, 500);

    assert_eq!(result, "2024-02-29 13:30:45.500");
}

#[test]
fn test_format_timestamp_year_boundary() {
    // 2023-12-31 23:59:59
    let result = format_timestamp(1704067199, 1);

    assert_eq!(result, "2023-12-31 23:59:59.001");
}

#[test]
fn test_format_timestamp_epoch() {
    assert_eq!(format_timestamp(0, 0), "1970-01-01 00:00:00.000");
}

#[test]
fn test_format_timestamp_epoch_millis() {
    assert_eq!(format_timestamp(0, 123), "1970-01-01 00:00:00.123");
}

#[test]
fn test_format_timestamp_millis_padding() {
    assert_eq!(format_timestamp(0, 5), "1970-01-01 00:00:00.005");
}

#[test]
fn test_timestamp_is_reasonable() {
    let timestamp = get_timestamp();

    // YYYY-MM-DD HH:MM:SS.mmm
    assert_eq!(timestamp.len(), 23);

    assert_eq!(&timestamp[4..5], "-");
    assert_eq!(&timestamp[7..8], "-");
    assert_eq!(&timestamp[10..11], " ");
    assert_eq!(&timestamp[13..14], ":");
    assert_eq!(&timestamp[16..17], ":");
    assert_eq!(&timestamp[19..20], ".");
}
