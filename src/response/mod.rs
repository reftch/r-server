use std::collections::HashMap;

pub mod builder;
pub mod content_type;
pub mod status;

use crate::info;
use crate::response::builder::ResponseBuilder;
use crate::server::connection::ConnectionMetadata;

pub use self::content_type::ContentType;
pub use self::status::Status;

#[derive(Debug)]
pub struct Response<'a, T> {
    pub status: Status,
    pub body: Vec<u8>,
    pub content_type: ContentType,
    pub headers: HashMap<String, String>,
    pub metadata: &'a ConnectionMetadata<T>,
}

impl<'a, T> Response<'a, T> {
    pub fn new(
        metadata: &'a ConnectionMetadata<T>,
        status: Status,
        body: impl Into<Vec<u8>>,
        content_type: ContentType,
    ) -> Self {
        Self {
            status,
            body: body.into(),
            content_type,
            headers: HashMap::new(),
            metadata,
        }
    }

    pub fn build(self) -> Vec<u8> {
        ResponseBuilder::new(self).build()
    }

    pub fn header<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.headers.entry(key.into()).or_insert(value.into());
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

    pub fn send(&mut self, body: String) -> &mut Self {
        info!("Not implemented, echo: {}", body);
        self
    }
}

#[cfg(test)]
mod tests;
