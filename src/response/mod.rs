use std::collections::HashMap;
use std::io::{self, Write};
use std::net::TcpStream;

pub mod builder;
pub mod content_type;
pub mod status;

use crate::info;
use crate::response::builder::ResponseBuilder;
use crate::server::connection::ConnectionMetadata;
use crate::server::https::TlsState;

pub use self::content_type::ContentType;
pub use self::status::Status;

pub trait SseWriter {
    fn write_sse(&self, data: &[u8]) -> io::Result<()>;
}

impl SseWriter for TcpStream {
    fn write_sse(&self, data: &[u8]) -> io::Result<()> {
        // self.write(data);
        let mut stream = self.try_clone()?;
        // stream = self.write(data);
        stream.write_all(data)
    }
}

impl SseWriter for Option<TlsState> {
    fn write_sse(&self, data: &[u8]) -> io::Result<()> {
        match self {
            Some(TlsState::Connected(stream)) => {
                let mut stream = stream.get_ref().try_clone()?;
                stream.write_all(data)
            }

            Some(TlsState::Handshaking(stream)) => {
                let mut stream = stream.get_ref().try_clone()?;
                stream.write_all(data)
            }

            None => Ok(()),
        }
    }
}

#[derive(Debug, Clone)]
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

    pub fn enable_sse(&mut self) -> &mut Self {
        self.content_type = ContentType::SSE;

        self.header("Cache-Control", "no-cache");
        self.header("Connection", "keep-alive");
        self.header("Content-Type", "text/event-stream");

        self
    }

    pub fn send(&mut self, body: String) -> &mut Self {
        info!("Not implemented, echo: {}", body);
        self
    }
}

impl<'a, T> Response<'a, T>
where
    T: SseWriter,
{
    pub fn sse(&self, data: &str) -> io::Result<()> {
        let payload = format!("data: {}\n\n", data);

        // 1. Define the headers and status line
        // 2. Use \r\n for proper HTTP line endings
        // 3. Ensure there is a double \r\n before the first data chunk begins
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
               Content-Type: text/event-stream\r\n\
               Cache-Control: no-cache\r\n\
               Connection: keep-alive\r\n\
               \r\n\
               {}\n\n",
            payload
        );

        // let res = self.build();
        // match conn.metadata.stream.write(&res) {}
        // self.metadata.stream.write_sse(res)
        self.metadata.stream.write_sse(response.as_bytes())
    }
}

#[cfg(test)]
mod tests;
