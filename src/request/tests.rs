use crate::request::Request;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_parse_valid() {
        let buf = b"GET / HTTP/1.1\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.method, "GET".into());
        assert_eq!(request.path, "/".into());
        assert_eq!(request.version, "HTTP/1.1".into());
    }

    #[test]
    fn test_request_parse_headers() {
        let buf =
            b"POST / HTTP/1.1\r\nContent-Type: application/json\r\nX-Custom-Header: value\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.method, "POST".into());
        assert_eq!(request.path, "/".into());
        assert_eq!(request.header("Content-Type"), Some("application/json"));
        assert_eq!(request.header("X-Custom-Header"), Some("value"));
    }

    #[test]
    fn test_request_parse_invalid() {
        let buf = b"GET / HTTP/1.1\r\n";
        let request = Request::parse(buf);
        assert!(request.is_none());
    }

    #[test]
    fn test_request_mime_type_case_insensitive() {
        let buf = b"POST / HTTP/1.1\r\ncontent-type: image/png\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.mime_type(), Some("image/png".to_string()));

        let buf2 = b"POST / HTTP/1.1\r\nContent-Type: image/png\r\n\r\n";
        let request2 = Request::parse(buf2).expect("Should parse valid request");
        assert_eq!(request2.mime_type(), Some("image/png".to_string()));
    }

    #[test]
    fn test_request_mime_type_none() {
        let buf = b"GET / HTTP/1.1\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.mime_type(), None);
    }

    #[test]
    fn test_request_header_trimming() {
        let buf = b"GET / HTTP/1.1\r\nHeader-Key:   value  \r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.header("Header-Key"), Some("value"));
    }

    #[test]
    fn test_request_empty_path() {
        // This might be a bit of an edge case for the current parser
        let buf = b"GET / HTTP/1.1\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.path, "/".into());
    }

    // #[test]
    #[test]
    fn test_request_malformed_request_line() {
        // Missing method
        let buf = b" / HTTP/1.1\r\n\r\n";
        let request = Request::parse(buf);
        assert!(request.is_none());

        // Missing path
        let buf2 = b"GET \r\n\r\n";
        let request2 = Request::parse(buf2);
        assert!(request2.is_none());

        // Missing second space
        let buf4 = b"GET / \r\n\r\n";
        let request4 = Request::parse(buf4);
        assert!(request4.is_none());
    }

    #[test]
    fn test_request_parse_query_params() {
        let buf = b"GET /path?name=value&age=30 HTTP/1.1\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.path, "/path".into());
        assert_eq!(request.query("name"), Some("value"));
        assert_eq!(request.query("age"), Some("30"));
    }

    #[test]
    fn test_request_parse_malformed_headers() {
        // Header without colon should be skipped according to current implementation
        let buf = b"GET / HTTP/1.1\r\nInvalidHeaderLine\r\nX-Valid: value\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.headers.len(), 1);
        assert_eq!(request.header("X-Valid"), Some("value"));
    }

    /// Deterministic pseudo-random bytes (xorshift64*), so tests exercise
    /// realistic binary payloads without external dependencies.
    fn pseudo_random_bytes(seed: u64, len: usize) -> Vec<u8> {
        let mut state = seed;
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 32) as u8
            })
            .collect()
    }

    /// Builds a browser-style multipart upload HTTP request.
    /// Returns (raw request bytes, raw multipart body bytes).
    fn build_binary_upload_request(file_data: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let boundary = "----WebKitFormBoundaryABC123";

        let mut body = Vec::new();
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"img.png\"\r\n",
        );
        body.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
        body.extend_from_slice(file_data);
        body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

        let mut http = Vec::new();
        http.extend_from_slice(b"POST /upload HTTP/1.1\r\n");
        http.extend_from_slice(
            format!(
                "Content-Type: multipart/form-data; boundary={}\r\n",
                boundary
            )
            .as_bytes(),
        );
        http.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
        http.extend_from_slice(&body);
        (http, body)
    }

    #[test]
    fn test_request_binary_multipart_upload_roundtrip() {
        // PNG magic followed by pseudo-random binary data including \r and \n
        let mut file_data = Vec::new();
        file_data.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        file_data.extend_from_slice(&pseudo_random_bytes(0xC0FFEE, 16 * 1024));

        let (http, mp_body) = build_binary_upload_request(&file_data);
        let request = Request::parse(&http).expect("Should parse upload request");

        assert_eq!(request.method, "POST".into());
        assert_eq!(request.path, "/upload".into());
        assert_eq!(request.body, mp_body);

        let fields = request.get_form_fields().expect("form parse");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "file");
        assert_eq!(fields[0].filename.as_deref(), Some("img.png"));
        assert_eq!(fields[0].content_type.as_deref(), Some("image/png"));
        assert_eq!(fields[0].data, file_data, "uploaded binary corrupted");

        let file = request.get_form_file("file").expect("file field");
        assert_eq!(file.data, file_data);

        assert_eq!(
            request.get_form_field("file"),
            Err("'file' is a file upload; use get_form_file('file')".to_string())
        );
    }

    #[test]
    fn test_request_incomplete_body_returns_none() {
        let file_data = pseudo_random_bytes(7, 4096);
        let (http, mp_body) = build_binary_upload_request(&file_data);
        let total_body = mp_body.len();
        let headers_len = http.len() - total_body;

        // Deliver only headers plus part of the body: request must NOT be
        // dispatched until every announced byte has arrived.
        for arrived in [0usize, 10, total_body / 4, total_body / 2, total_body - 1] {
            let cut = headers_len + arrived;
            let parsed = Request::parse(&http[..cut]);
            assert!(
                parsed.is_none(),
                "dispatched early with {arrived}/{total_body} body bytes"
            );
        }

        // Complete body -> parses successfully with exact payload.
        let parsed = Request::parse(&http).expect("complete request should parse");
        assert_eq!(parsed.body, mp_body);
    }

    #[test]
    fn test_request_content_length_truncates_extra_bytes() {
        let (mut http, mp_body) = build_binary_upload_request(b"\x00\x01\x02\r\n\x03");

        // Simulate an extra byte appended beyond Content-Length
        http.extend_from_slice(b"EXTRA");

        let request = Request::parse(&http).expect("Should parse request");
        assert_eq!(request.body, mp_body);
    }

    #[test]
    fn test_request_without_content_length_keeps_legacy_behavior() {
        // No Content-Length header -> body is whatever follows the headers
        let buf = b"POST / HTTP/1.1\r\nContent-Type: text/plain\r\n\r\nraw bytes";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.body, b"raw bytes");
    }

    #[test]
    fn test_request_zero_content_length() {
        let buf = b"POST / HTTP/1.1\r\nContent-Length: 0\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert!(request.body.is_empty());
    }

    #[test]
    fn test_request_case_insensitive_content_length() {
        let buf = b"POST / HTTP/1.1\r\ncontent-LENGTH: 5\r\n\r\nhello";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.body, b"hello");
    }

    fn build_form_request(body: &str) -> Request {
        let raw = format!(
            "POST / HTTP/1.1\r\n\
             Content-Type: application/x-www-form-urlencoded\r\n\
             Content-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        Request::parse(raw.as_bytes()).expect("Should parse form request")
    }

    #[test]
    fn test_request_get_form_fields() {
        let request = build_form_request("name=Alice&age=30");
        let fields = request.get_form_fields().expect("form parse");

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "name");
        assert_eq!(fields[0].text(), "Alice");
        assert_eq!(fields[0].filename, None);
        assert_eq!(fields[1].name, "age");
        assert_eq!(fields[1].data, b"30");

        assert_eq!(request.get_form_field("name"), Ok("Alice".to_string()));
        assert_eq!(request.get_form_field("age"), Ok("30".to_string()));
    }

    #[test]
    fn test_request_get_form_field_percent_decoding() {
        let request = build_form_request("full=Alice+Smith&sym=%21%40%23&empty=&nokey&x=a%3Db");
        let fields = request.get_form_fields().expect("form parse");
        assert_eq!(fields.len(), 5);

        assert_eq!(
            request.get_form_field("full"),
            Ok("Alice Smith".to_string())
        );
        assert_eq!(request.get_form_field("sym"), Ok("!@#".to_string()));
        assert_eq!(request.get_form_field("empty"), Ok(String::new()));
        assert_eq!(request.get_form_field("nokey"), Ok(String::new()));
        assert_eq!(request.get_form_field("x"), Ok("a=b".to_string()));
    }

    #[test]
    fn test_request_get_form_field_missing() {
        let request = build_form_request("name=Alice");
        assert_eq!(
            request.get_form_field("missing"),
            Err("Missing 'missing' field".to_string())
        );
    }

    #[test]
    fn test_request_get_form_fields_wrong_content_type() {
        let buf =
            b"POST / HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
        let request = Request::parse(buf).expect("Should parse valid request");

        assert!(request.get_form_fields().is_err());
        assert!(request.get_form_field("name").is_err());
    }

    #[test]
    fn test_request_get_form_fields_empty_body() {
        let buf = b"POST / HTTP/1.1\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 0\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");

        assert!(request.get_form_fields().unwrap().is_empty());
    }

    #[test]
    fn test_request_unified_form_api_on_multipart() {
        let boundary = "UNIFIED";
        let mut body = Vec::new();
        body.extend_from_slice(
            b"--UNIFIED\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\nAlice\r\n",
        );
        body.extend_from_slice(
            b"--UNIFIED\r\n\
             Content-Disposition: form-data; name=\"avatar\"; filename=\"a.png\"\r\n\
             Content-Type: image/png\r\n\r\nPNGDATA\r\n",
        );
        body.extend_from_slice(b"--UNIFIED--\r\n");

        let raw = format!(
            "POST /upload HTTP/1.1\r\n\
             Content-Type: multipart/form-data; boundary={}\r\n\
             Content-Length: {}\r\n\r\n",
            boundary,
            body.len()
        );
        let mut raw = raw.into_bytes();
        raw.extend_from_slice(&body);

        let request = Request::parse(&raw).expect("Should parse multipart request");
        let fields = request.get_form_fields().expect("form parse");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].text(), "Alice");

        assert_eq!(
            request.get_form_field("name"),
            Ok("Alice".to_string()),
            "same accessor as urlencoded forms"
        );

        let file = request.get_form_file("avatar").expect("file field");
        assert_eq!(file.filename.as_deref(), Some("a.png"));
        assert_eq!(file.content_type.as_deref(), Some("image/png"));
        assert_eq!(file.data, b"PNGDATA");

        assert_eq!(
            request.get_form_field("avatar"),
            Err("'avatar' is a file upload; use get_form_file('avatar')".to_string())
        );
        assert_eq!(
            request.get_form_file("name"),
            Err("'name' is a text field; use get_form_field('name')".to_string())
        );
    }

    #[test]
    fn test_request_get_form_file_on_urlencoded_body() {
        let request = build_form_request("name=Alice");

        assert_eq!(
            request.get_form_file("name"),
            Err("'name' is a text field; use get_form_field('name')".to_string())
        );
    }

    #[test]
    fn test_request_get_form_file_no_file_selected() {
        // Browsers submit file inputs with `filename=""` when nothing was picked
        let boundary = "NOFILE";
        let mut body = Vec::new();
        body.extend_from_slice(
            b"--NOFILE\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n\r\n",
        );
        body.extend_from_slice(b"--NOFILE--\r\n");

        let raw = format!(
            "POST / HTTP/1.1\r\n\
             Content-Type: multipart/form-data; boundary={}\r\n\
             Content-Length: {}\r\n\r\n",
            boundary,
            body.len()
        );
        let mut raw = raw.into_bytes();
        raw.extend_from_slice(&body);

        let request = Request::parse(&raw).expect("Should parse multipart request");
        assert_eq!(
            request.get_form_file("file"),
            Err("No file selected for 'file'".to_string())
        );
    }

    #[test]
    fn test_request_unsupported_form_content_type_message() {
        let buf =
            b"POST / HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
        let request = Request::parse(buf).expect("Should parse valid request");

        assert_eq!(
            request.get_form_fields(),
            Err("unsupported form content type 'application/json'".to_string())
        );
    }
}
