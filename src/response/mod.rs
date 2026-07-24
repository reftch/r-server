use std::collections::HashMap;

pub mod builder;
pub mod content_type;
pub mod status;

use crate::response::builder::ResponseBuilder;

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

    pub fn build(self) -> Vec<u8> {
        ResponseBuilder::new(self).build()
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
