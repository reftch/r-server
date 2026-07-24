use std::collections::HashMap;

pub mod content_type;
pub mod status;

pub use self::content_type::ContentType;
pub use self::status::Status;

#[derive(Debug, PartialEq)]
pub struct Response {
    pub status: Status,
    pub body: Vec<u8>,
    pub content_type: ContentType,
    pub headers: HashMap<String, String>,
}

impl Response {
    pub fn new(status: Status, body: impl Into<Vec<u8>>, content_type: ContentType) -> Self {
        Self {
            status,
            body: body.into(),
            content_type,
            headers: HashMap::new(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {} {}\r\n",
            self.status.as_u16(),
            self.status.reason_phrase()
        );

        // Add Content-Type header
        response.push_str(&format!("Content-Type: {}\r\n", self.content_type.as_str()));

        // Add Content-Length header
        response.push_str(&format!("Content-Length: {}\r\n", self.body.len()));

        // Add custom headers from the collection
        for (key, value) in &self.headers {
            response.push_str(&format!("{}: {}\r\n", key, value));
        }

        response.push_str("\r\n");
        let mut response_bytes = response.into_bytes();
        response_bytes.extend(&self.body);
        response_bytes
    }

    pub fn header(&mut self, key: String, value: String) -> &mut Self {
        self.headers.entry(key).or_insert(value);
        self
    }

    pub fn status(&mut self, status: Status) -> &mut Self {
        self.status = status;
        self
    }

    pub fn body(&mut self, body: impl Into<Vec<u8>>) -> &mut Self {
        self.body = body.into();
        self
    }

    pub fn content_type(&mut self, content_type: ContentType) -> &mut Self {
        self.content_type = content_type;
        self
    }
}

#[cfg(test)]
mod tests;
