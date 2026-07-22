use crate::response::{ContentType, Response, Status};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_new() {
        let response = Response::new(Status::Ok, "Hello World", ContentType::TEXT);
        assert_eq!(response.status, Status::Ok);
        assert_eq!(response.body, b"Hello World".to_vec());
        assert_eq!(response.content_type, ContentType::TEXT);
    }

    #[test]
    fn test_response_to_bytes() {
        let response = Response::new(Status::Ok, "OK", ContentType::TEXT);
        let bytes = response.to_bytes();
        let bytes_str = String::from_utf8(bytes).unwrap();
        assert!(bytes_str.contains("HTTP/1.1 200 OK"));
        assert!(bytes_str.contains("Content-Length: 2"));
        assert!(bytes_str.ends_with("OK"));
        assert_eq!(response.content_type, ContentType::TEXT);
    }

    #[test]
    fn test_content_type_as_str() {
        assert_eq!(ContentType::HTML.as_str(), "text/html");
        assert_eq!(ContentType::JSON.as_str(), "application/json");
        assert_eq!(ContentType::UNKNOWN.as_str(), "application/octet-stream");
    }

    #[test]
    fn test_response_add_header() {
        let mut response = Response::new(Status::Ok, "OK", ContentType::TEXT);
        response.set_header("X-Test".to_string(), "Value".to_string());
        assert_eq!(response.headers.get("X-Test").unwrap(), "Value");

        // Ensure duplicate headers are not added (as per implementation)
        response.set_header("X-Test".to_string(), "New Value".to_string());
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
        let response = Response::new(Status::NotFound, "Not Found", ContentType::TEXT);
        let bytes = response.to_bytes();
        let bytes_str = String::from_utf8(bytes).unwrap();
        assert!(bytes_str.contains("HTTP/1.1 404 Not Found"));
        assert_eq!(response.content_type, ContentType::TEXT);
    }

    #[test]
    fn test_response_to_bytes_with_headers() {
        let mut response = Response::new(Status::Ok, "OK", ContentType::TEXT);
        response.set_header("Custom-Header".to_string(), "Custom-Value".to_string());
        let bytes = response.to_bytes();
        let bytes_str = String::from_utf8(bytes).unwrap();
        assert!(bytes_str.contains("Custom-Header: Custom-Value\r\n"));
    }
}
