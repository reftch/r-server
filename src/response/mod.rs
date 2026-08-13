use std::collections::HashMap;
use std::io;

pub mod builder;
pub mod content_type;
pub mod status;
pub mod stream;

use crate::response::builder::ResponseBuilder;
use crate::response::stream::StreamWriter;
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

impl<'a, T> Clone for Response<'a, T> {
    fn clone(&self) -> Self {
        Self {
            status: self.status,
            body: self.body.clone(),
            content_type: ContentType::SSE,
            headers: self.headers.clone(),
            metadata: self.metadata,
        }
    }
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
}

impl<'a, T> Response<'a, T>
where
    T: StreamWriter,
{
    pub fn stream(&self, data: &str) -> io::Result<()> {
        let payload = format!("data: {}\n\n", data);

        // Define the headers and status line
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
               Content-Type: text/event-stream\r\n\
               Cache-Control: no-cache\r\n\
               Connection: keep-alive\r\n\
               \r\n\
               {}\n\n",
            payload
        );

        // Write the response to the stream
        self.metadata.stream.write(response.as_bytes())
    }
}

#[cfg(test)]
mod tests;
