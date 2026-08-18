use crate::response::stream::StreamWriter;
use crate::response::{ContentType, Response, Status};
use crate::server::connection::{ConnectionMetadata, ConnectionStreamClone};
use std::io;
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Default)]
    struct TestStream {
        data: Arc<Mutex<Vec<u8>>>,
    }

    impl StreamWriter for TestStream {
        fn write(&self, data: &[u8]) -> io::Result<()> {
            self.data
                .lock()
                .map_err(|_| io::Error::other("test stream mutex poisoned"))?
                .extend_from_slice(data);

            Ok(())
        }
    }

    impl ConnectionStreamClone for TestStream {
        fn clone_stream(&self) -> io::Result<Self> {
            Ok(self.clone())
        }
    }

    fn metadata() -> ConnectionMetadata<TestStream> {
        ConnectionMetadata {
            stream: TestStream::default(),
        }
    }

    #[test]
    fn test_response_new() {
        let metadata = metadata();

        let response = Response::new(&metadata, Status::Ok, "Hello World", ContentType::TEXT);

        assert_eq!(response.status, Status::Ok);
        assert_eq!(response.body, b"Hello World".to_vec());
        assert_eq!(response.content_type, ContentType::TEXT);
    }

    #[test]
    fn test_response_to_bytes() {
        let metadata = metadata();

        let response = Response::new(&metadata, Status::Ok, "OK", ContentType::TEXT);

        let bytes = response.build();
        let bytes_str = String::from_utf8(bytes).unwrap();

        assert!(bytes_str.contains("HTTP/1.1 200 OK"));
        assert!(bytes_str.contains("Content-Length: 2"));
        assert!(bytes_str.ends_with("OK"));
    }

    #[test]
    fn test_content_type_as_str() {
        assert_eq!(ContentType::HTML.as_str(), "text/html");
        assert_eq!(ContentType::JSON.as_str(), "application/json");
        assert_eq!(ContentType::UNKNOWN.as_str(), "application/octet-stream");
    }

    #[test]
    fn test_response_add_header() {
        let metadata = metadata();

        let mut response = Response::new(&metadata, Status::Ok, "OK", ContentType::TEXT);

        response.header("X-Test".to_string(), "Value".to_string());

        assert_eq!(response.headers.get("X-Test").unwrap(), "Value");

        // Ensure duplicate headers are not added.
        response.header("X-Test".to_string(), "New Value".to_string());

        assert_eq!(response.headers.get("X-Test").unwrap(), "Value");
    }

    #[test]
    fn test_status_helpers() {
        assert_eq!(Status::Ok.as_u16(), 200);
        assert_eq!(Status::NotFound.as_u16(), 404);
        assert_eq!(Status::InternalServerError.as_u16(), 500);

        assert_eq!(Status::Ok.reason_phrase(), "OK");
        assert_eq!(Status::NotFound.reason_phrase(), "Not Found");
        assert_eq!(
            Status::InternalServerError.reason_phrase(),
            "Internal Server Error"
        );
    }

    #[test]
    fn test_response_404() {
        let metadata = metadata();

        let response = Response::new(&metadata, Status::NotFound, "Not Found", ContentType::TEXT);

        let bytes = response.build();
        let bytes_str = String::from_utf8(bytes).unwrap();

        assert!(bytes_str.contains("HTTP/1.1 404 Not Found"));
    }

    #[test]
    fn test_response_to_bytes_with_headers() {
        let metadata = metadata();

        let mut response = Response::new(&metadata, Status::Ok, "OK", ContentType::TEXT);

        response.header("Custom-Header".to_string(), "Custom-Value".to_string());

        let bytes = response.build();
        let bytes_str = String::from_utf8(bytes).unwrap();

        assert!(bytes_str.contains("Custom-Header: Custom-Value\r\n"));
    }

    #[test]
    fn test_response_stream() {
        let metadata = metadata();

        let response = Response::new(&metadata, Status::Ok, "", ContentType::SSE);

        response.stream("Hello").unwrap();

        let written = metadata.stream.data.lock().unwrap().clone();

        let written = String::from_utf8(written).unwrap();

        assert!(written.contains("HTTP/1.1 200 OK"));
        assert!(written.contains("Content-Type: text/event-stream"));
        assert!(written.contains("data: Hello"));
    }

    #[test]
    fn test_response_flush() {
        let metadata = metadata();

        let mut response = Response::new(&metadata, Status::Ok, "Hello", ContentType::TEXT);

        response.header("X-Test", "Value");

        response.flush().unwrap();

        let written = metadata.stream.data.lock().unwrap().clone();

        let written = String::from_utf8(written).unwrap();

        assert!(written.contains("HTTP/1.1 200 OK"));
        assert!(written.contains("Content-Length: 5"));
        assert!(written.contains("X-Test: Value"));
        assert!(written.ends_with("Hello"));
    }
}
