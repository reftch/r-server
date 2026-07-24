use crate::response::Response;

pub struct ResponseBuilder {
    response: Response,
}

impl ResponseBuilder {
    pub fn new(response: Response) -> Self {
        Self { response }
    }

    /// This contains the logic previously in to_bytes()
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

        line.push_str("\r\n");

        let mut full_response = line.into_bytes();
        full_response.extend(&self.response.body);
        full_response
    }
}
