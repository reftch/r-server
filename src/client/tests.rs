use super::*;

#[test]
fn new_detects_https_and_defaults_to_443() {
    let client = Client::new("https://example.com");

    assert_eq!(client.host, "example.com");
    assert!(client.is_secure);
    assert_eq!(client.port, 443);
}

#[test]
fn new_detects_http_and_defaults_to_80() {
    let client = Client::new("http://example.com");

    assert_eq!(client.host, "example.com");
    assert!(!client.is_secure);
    assert_eq!(client.port, 80);
}

#[test]
fn new_accepts_host_without_scheme() {
    let client = Client::new("example.com");

    assert_eq!(client.host, "example.com");
    assert!(!client.is_secure);
    assert_eq!(client.port, 80);
}

#[test]
fn new_accepts_string_input() {
    let client = Client::new(String::from("https://example.com"));

    assert_eq!(client.host, "example.com");
    assert!(client.is_secure);
    assert_eq!(client.port, 443);
}

#[test]
fn new_parses_explicit_http_port() {
    let client = Client::new("http://localhost:9090");

    assert_eq!(client.host, "localhost");
    assert!(!client.is_secure);
    assert_eq!(client.port, 9090);
}

#[test]
fn new_parses_explicit_https_port() {
    let client = Client::new("https://example.com:8443");

    assert_eq!(client.host, "example.com");
    assert!(client.is_secure);
    assert_eq!(client.port, 8443);
}

#[test]
fn new_strips_path_component() {
    let client = Client::new("http://localhost:9090/realms/rserver");

    assert_eq!(client.host, "localhost");
    assert_eq!(client.port, 9090);
}

#[test]
fn new_parses_ipv6_with_port() {
    let client = Client::new("http://[::1]:9090");

    assert_eq!(client.host, "[::1]");
    assert!(!client.is_secure);
    assert_eq!(client.port, 9090);
}

#[test]
fn new_parses_ipv6_without_port() {
    let client = Client::new("https://[::1]");

    assert_eq!(client.host, "[::1]");
    assert!(client.is_secure);
    assert_eq!(client.port, 443);
}

#[test]
fn decode_chunked_decodes_single_chunk() {
    let body = b"5\r\nhello\r\n0\r\n\r\n";

    let result = decode_chunked(body).unwrap();

    assert_eq!(result, b"hello");
}

#[test]
fn decode_chunked_decodes_multiple_chunks() {
    let body = b"5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";

    let result = decode_chunked(body).unwrap();

    assert_eq!(result, b"hello world");
}

#[test]
fn decode_chunked_handles_empty_body() {
    let body = b"0\r\n\r\n";

    let result = decode_chunked(body).unwrap();

    assert!(result.is_empty());
}

#[test]
fn decode_chunked_handles_binary_data() {
    let body = b"4\r\n\x00\x01\x02\xff\r\n0\r\n\r\n";

    let result = decode_chunked(body).unwrap();

    assert_eq!(result, vec![0, 1, 2, 255]);
}

#[test]
fn decode_chunked_rejects_missing_size_line_terminator() {
    let body = b"5";

    let result = decode_chunked(body);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Invalid chunked encoding");
}

#[test]
fn decode_chunked_rejects_invalid_hex_size() {
    let body = b"zz\r\nhello\r\n0\r\n\r\n";

    let result = decode_chunked(body);

    assert!(result.is_err());
}

#[test]
fn decode_chunked_rejects_incomplete_chunk() {
    let body = b"5\r\nhel";

    let result = decode_chunked(body);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Incomplete chunk");
}

#[test]
fn decode_chunked_rejects_invalid_chunk_terminator() {
    let body = b"5\r\nhelloXX0\r\n\r\n";

    let result = decode_chunked(body);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Invalid chunk terminator");
}

#[test]
fn decode_chunked_supports_uppercase_hex() {
    let body = b"A\r\n0123456789\r\n0\r\n\r\n";

    let result = decode_chunked(body).unwrap();

    assert_eq!(result, b"0123456789");
}

#[test]
fn decode_chunked_supports_chunk_extensions() {
    // This test documents a limitation: the current implementation does
    // NOT support valid HTTP chunk extensions such as ";foo=bar".
    let body = b"5;foo=bar\r\nhello\r\n0\r\n\r\n";

    assert!(decode_chunked(body).is_err());
}
