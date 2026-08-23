use crate::core::connection::{ConnectionMetadata, ConnectionStreamClone};
use crate::response::stream::StreamWriter;
use crate::response::{ContentType, Response, Status};
use std::io;
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod tests {
    use crate::core::metadata::Metadata;

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

    // Implement Write for &TestStream using internal mutability
    impl std::io::Write for &TestStream {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.data.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl ConnectionStreamClone for TestStream {
        fn clone_stream(&self) -> io::Result<Self> {
            Ok(self.clone())
        }
    }

    fn metadata() -> Arc<dyn Metadata> {
        Arc::new(ConnectionMetadata {
            stream: Arc::new(TestStream::default()),
        })
    }

    #[test]
    fn test_response_new() {
        let metadata = metadata();

        let response = Response::new(
            metadata, // Pass by reference rather than wrapping in Arc
            Status::Ok,
            b"Hello World", // Use a byte string literal (b"...") instead of a standard string
            ContentType::TEXT,
        );

        assert_eq!(response.status, Status::Ok);
        assert_eq!(response.body, b"Hello World".to_vec());
        assert_eq!(response.content_type, ContentType::TEXT);
    }

    #[test]
    fn test_response_to_bytes() {
        let metadata = metadata();

        let response = Response::new(metadata, Status::Ok, "OK", ContentType::TEXT);

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

        let mut response = Response::new(metadata, Status::Ok, "OK", ContentType::TEXT);

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

        let response = Response::new(metadata, Status::NotFound, "Not Found", ContentType::TEXT);

        let bytes = response.build();
        let bytes_str = String::from_utf8(bytes).unwrap();

        assert!(bytes_str.contains("HTTP/1.1 404 Not Found"));
    }

    #[test]
    fn test_response_to_bytes_with_headers() {
        let metadata = metadata();

        let mut response = Response::new(metadata, Status::Ok, "OK", ContentType::TEXT);

        response.header("Custom-Header".to_string(), "Custom-Value".to_string());

        let bytes = response.build();
        let bytes_str = String::from_utf8(bytes).unwrap();

        assert!(bytes_str.contains("Custom-Header: Custom-Value\r\n"));
    }

    #[test]
    fn test_response_stream() {
        let stream = Arc::new(TestStream::default());
        let metadata = Arc::new(ConnectionMetadata {
            stream: stream.clone(),
        });

        // 1. Declare response as mutable
        let response = Response::new(metadata, Status::Ok, b"", ContentType::SSE);

        // 2. Call stream with &mut response
        response.stream("Hello").unwrap();

        let written = stream.data.lock().unwrap().clone();
        let written = String::from_utf8(written).unwrap();

        assert!(written.contains("HTTP/1.1 200 OK"));
        assert!(written.contains("Content-Type: text/event-stream"));
        assert!(written.contains("data: Hello"));
    }

    #[test]
    fn test_response_flush() {
        // 1. Create concrete stream and metadata
        let stream = Arc::new(TestStream::default());
        let meta = Arc::new(ConnectionMetadata {
            stream: stream.clone(),
        });

        // 2. Pass byte slice b"Hello" and the Arc metadata
        let mut response = Response::new(meta, Status::Ok, b"Hello", ContentType::TEXT);

        response.header("X-Test", "Value");
        response.flush().unwrap();

        // 3. Inspect data directly from the concrete stream reference
        let written = stream.data.lock().unwrap().clone();
        let written = String::from_utf8(written).unwrap();

        assert!(written.contains("HTTP/1.1 200 OK"));
        assert!(written.contains("Content-Length: 5"));
        assert!(written.contains("X-Test: Value"));
        assert!(written.ends_with("Hello"));
    }
}
