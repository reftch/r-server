use crate::response::Response;

pub struct ResponseBuilder<'a> {
    response: Response<'a>,
}

impl<'a> ResponseBuilder<'a> {
    pub fn new(response: Response<'a>) -> Self {
        Self { response }
    }

    pub fn build(self) -> Vec<u8> {
        let mut line = format!(
            "HTTP/1.1 {} {}\r\n",
            self.response.status.as_u16(),
            self.response.status.reason_phrase()
        );

        // Add Content-Type header
        line.push_str(&format!(
            "Content-Type: {}\r\n",
            self.response.content_type.as_str()
        ));

        // Add Content-Length header
        line.push_str(&format!("Content-Length: {}\r\n", self.response.body.len()));

        // Add custom headers
        for (key, value) in &self.response.headers {
            line.push_str(&format!("{}: {}\r\n", key, value));
        }

        // End headers
        line.push_str("\r\n");

        // Build complete response
        let mut full_response = line.into_bytes();
        full_response.extend_from_slice(&self.response.body);

        full_response
    }
}
