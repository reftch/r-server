use std::collections::HashMap;
use std::{fmt, io};

pub mod builder;
pub mod content_type;
pub mod status;
pub mod stream;

use crate::response::builder::ResponseBuilder;
use crate::server::metadata::Metadata;

pub use self::content_type::ContentType;
pub use self::status::Status;

pub struct Response<'a> {
    pub status: Status,
    pub body: Vec<u8>,
    pub content_type: ContentType,
    pub headers: HashMap<String, String>,
    pub metadata: &'a dyn Metadata,
}

impl<'a> fmt::Debug for Response<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("body", &self.body)
            .field("content_type", &self.content_type)
            .field("headers", &self.headers)
            .field("metadata", &"<omitted>")
            .finish()
    }
}

impl<'a> Clone for Response<'a> {
    fn clone(&self) -> Self {
        Self {
            status: self.status,
            body: self.body.clone(),
            content_type: self.content_type.clone(),
            headers: self.headers.clone(),
            metadata: self.metadata,
        }
    }
}

impl<'a> Response<'a> {
    pub fn new(
        metadata: &'a dyn Metadata,
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

    pub fn stream(&self, data: &str) -> io::Result<()> {
        let payload = format!("data: {}\n\n", data);

        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/event-stream\r\n\
             Cache-Control: no-cache\r\n\
             Connection: keep-alive\r\n\
             \r\n\
             {}\n\n",
            payload
        );

        self.metadata.write(response.as_bytes())
    }

    pub fn flush(&self) -> io::Result<()> {
        let response = self.clone().build();
        // write stream
        self.metadata.write(&response)
    }
}

#[cfg(test)]
mod tests;
