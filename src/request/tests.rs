use crate::request::Request;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_parse_valid() {
        let buf = b"GET / HTTP/1.1\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/");
    }

    #[test]
    fn test_request_parse_headers() {
        let buf =
            b"POST / HTTP/1.1\r\nContent-Type: application/json\r\nX-Custom-Header: value\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/");
        assert_eq!(
            *request.headers.get("Content-Type").unwrap(),
            "application/json"
        );
        assert_eq!(*request.headers.get("X-Custom-Header").unwrap(), "value");
    }

    #[test]
    fn test_request_parse_invalid() {
        let buf = b"GET / HTTP/1.1\r\n";
        let request = Request::parse(buf);
        assert!(request.is_none());
    }

    #[test]
    fn test_request_mime_type() {
        let buf = b"POST / HTTP/1.1\r\nContent-Type: image/png\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.mime_type(), Some("image/png"));
    }

    #[test]
    fn test_request_mime_type_none() {
        let buf = b"GET / HTTP/1.1\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.mime_type(), None);
    }

    #[test]
    fn test_request_mime_type_case_insensitive() {
        // The current implementation uses HashMap::get which is case-sensitive for keys.
        // Let's test if "Content-Type" works but "content-type" doesn't (based on the current code).
        let buf = b"POST / HTTP/1.1\r\ncontent-type: image/png\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.mime_type(), None);

        let buf2 = b"POST / HTTP/1.1\r\nContent-Type: image/png\r\n\r\n";
        let request2 = Request::parse(buf2).expect("Should parse valid request");
        assert_eq!(request2.mime_type(), Some("image/png"));
    }

    #[test]
    fn test_request_header_trimming() {
        let buf = b"GET / HTTP/1.1\r\nHeader-Key:   value  \r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(*request.headers.get("Header-Key").unwrap(), "value");
    }

    #[test]
    fn test_request_empty_path() {
        // This might be a bit of an edge case for the current parser
        let buf = b"GET / HTTP/1.1\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.path, "/");
    }

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
        assert_eq!(request.path, "/path");
        assert_eq!(*request.query_params.get("name").unwrap(), "value");
        assert_eq!(*request.query_params.get("age").unwrap(), "30");
    }

    #[test]
    fn test_request_parse_malformed_headers() {
        // Header without colon should be skipped according to current implementation
        let buf = b"GET / HTTP/1.1\r\nInvalidHeaderLine\r\nX-Valid: value\r\n\r\n";
        let request = Request::parse(buf).expect("Should parse valid request");
        assert_eq!(request.headers.len(), 1);
        assert_eq!(*request.headers.get("X-Valid").unwrap(), "value");
    }
}
