use crate::response::Response;

pub struct ResponseBuilder {
    response: Response,
}

impl ResponseBuilder {
    pub fn new(response: Response) -> Self {
        Self { response }
    }

    pub fn build(self) -> Vec<u8> {
        // Estimate header size to minimize re-allocations
        let estimated_header_size = 128 + (self.response.headers.len() * 64);
        let mut full_response =
            Vec::with_capacity(estimated_header_size + self.response.body.len());

        // Status Line
        full_response.extend_from_slice(
            format!(
                "HTTP/1.1 {} {}\r\n",
                self.response.status.as_u16(),
                self.response.status.reason_phrase()
            )
            .as_bytes(),
        );

        // Content-Type Header
        full_response.extend_from_slice(
            format!("Content-Type: {}\r\n", self.response.content_type.as_str()).as_bytes(),
        );

        // Content-Length Header
        full_response.extend_from_slice(
            format!("Content-Length: {}\r\n", self.response.body.len()).as_bytes(),
        );

        // Custom Headers
        for (key, value) in &self.response.headers {
            full_response.extend_from_slice(format!("{}: {}\r\n", key, value).as_bytes());
        }

        // End Headers
        full_response.extend_from_slice(b"\r\n");

        // Body
        full_response.extend_from_slice(&self.response.body);

        full_response
    }
}
