mod multipart;

use std::io::Cursor;

use memchr::memchr;

pub use multipart::MultipartField;

use crate::request::multipart::{extract_boundary_from_content_type, parse_multipart};

/// Represents an HTTP request without lifetime annotations.
pub struct Request {
    /// The HTTP method (e.g., "GET", "POST").
    pub method: Box<str>,
    /// The request path.
    pub path: Box<str>,
    /// The HTTP version (e.g., "HTTP/1.1").
    pub version: Box<str>,

    /// A list of request headers as key-value pairs.
    pub headers: Vec<KeyValuePair>,
    /// A list of route parameters.
    pub params: Vec<KeyValuePair>,
    /// A list of query parameters from the URL.
    pub query_params: Vec<KeyValuePair>,
    /// Raw body of request
    pub body: Vec<u8>,
}

/// Type alias for key-value string pairs used in headers and parameters.
pub type KeyValuePair = (Box<str>, Box<str>);

impl Request {
    /// Finds the end of the request headers by looking for the double CRLF.
    #[inline(always)]
    fn find_header_end(buf: &[u8]) -> Option<usize> {
        let mut i = 0;
        let len = buf.len();

        while i + 3 < len {
            if buf[i] == b'\r' && buf[i + 1] == b'\n' && buf[i + 2] == b'\r' && buf[i + 3] == b'\n'
            {
                return Some(i + 4);
            }

            i += 1;
        }

        None
    }

    /// Finds a header value by name (case-insensitive) in a parsed header list.
    #[inline(always)]
    fn find_header_value<'a>(headers: &'a [KeyValuePair], name: &str) -> Option<&'a str> {
        headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| &**v)
    }

    /// Parses the Content-Length header into a byte count, if present and valid.
    #[inline(always)]
    fn content_length(headers: &[KeyValuePair]) -> Option<usize> {
        Self::find_header_value(headers, "content-length")?
            .trim()
            .parse::<usize>()
            .ok()
    }

    /// Parses an HTTP request from a byte buffer.
    ///
    /// Returns `None` while the request is still incomplete (e.g. the body
    /// announced via Content-Length has not fully arrived), so buffered
    /// partial reads are kept until the whole request is available.
    #[inline(always)]
    pub fn parse(buf: &[u8]) -> Option<Self> {
        let header_end = Self::find_header_end(buf)?;

        // HTTP is ASCII. Avoid UTF-8 validation.
        let text = unsafe { std::str::from_utf8_unchecked(&buf[..header_end]) };

        let mut lines = text.split("\r\n");

        // Request line
        let request_line = lines.next()?;

        let bytes = request_line.as_bytes();

        let mut first_space = None;
        let mut second_space = None;

        for (i, &b) in bytes.iter().enumerate() {
            if b == b' ' {
                match first_space {
                    None => first_space = Some(i),
                    Some(_) => {
                        second_space = Some(i);
                        break;
                    }
                }
            }
        }

        let first_space = first_space?;
        let second_space = second_space?;

        let method = &request_line[..first_space];
        let version = &request_line[second_space + 1..];
        let full_path = &request_line[first_space + 1..second_space];
        if method.is_empty() || full_path.is_empty() || version != "HTTP/1.1" {
            return None;
        }

        let (path, query_params) = Self::parse_path_and_query(full_path);
        let headers = Self::parse_headers(&mut lines);

        // Wait until the body announced via Content-Length has fully arrived
        // before dispatching, otherwise large uploads (which span multiple
        // socket reads) would be handed to handlers with truncated bodies.
        let content_length = Self::content_length(&headers);
        if let Some(len) = content_length
            && buf.len() < header_end + len
        {
            return None;
        }

        let body_len = content_length.unwrap_or(buf.len() - header_end);

        Some(Self {
            method: method.into(),
            path: path.into(),
            version: version.into(),
            headers,
            params: Vec::with_capacity(4),
            query_params,
            body: buf[header_end..header_end + body_len].to_vec(),
        })
    }

    /// Parses the path and query parameters from a full path string.
    #[inline(always)]
    fn parse_path_and_query(full_path: &str) -> (&str, Vec<KeyValuePair>) {
        let Some(qpos) = memchr(b'?', full_path.as_bytes()) else {
            return (full_path, Vec::with_capacity(4));
        };

        let path = &full_path[..qpos];
        let mut params = Vec::with_capacity(4);

        let query = &full_path[qpos + 1..];
        let bytes = query.as_bytes();

        let mut start = 0;

        while start < bytes.len() {
            let end = memchr(b'&', &bytes[start..])
                .map(|i| start + i)
                .unwrap_or(bytes.len());

            if let Some(eq) = memchr(b'=', &bytes[start..end]) {
                let eq = start + eq;
                params.push((query[start..eq].into(), query[eq + 1..end].into()));
            }

            start = end + 1;
        }

        (path, params)
    }

    /// Parses headers from the provided lines.
    #[inline(always)]
    fn parse_headers(lines: &mut std::str::Split<'_, &str>) -> Vec<KeyValuePair> {
        let mut headers = Vec::with_capacity(12);

        for line in lines {
            if line.is_empty() {
                break;
            }

            let bytes = line.as_bytes();

            let Some(colon) = memchr::memchr(b':', bytes) else {
                continue;
            };

            let key = &line[..colon];

            let mut start = colon + 1;
            let mut end = bytes.len();

            // trim leading SP / HTAB
            while start < end && matches!(bytes[start], b' ' | b'\t') {
                start += 1;
            }

            // trim trailing SP / HTAB
            while end > start && matches!(bytes[end - 1], b' ' | b'\t') {
                end -= 1;
            }

            headers.push((key.into(), line[start..end].into()));
        }

        headers
    }

    /// Gets a parameter by name.
    #[inline(always)]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| &**k == name)
            .map(|(_, v)| &**v)
    }

    /// Gets a header by name.
    #[inline(always)]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| &**v)
    }

    /// Returns the raw Content-Type header value using case-insensitive lookup.
    pub fn content_type(&self) -> Option<&str> {
        self.header("content-type")
    }

    /// Gets a query parameter by name.
    #[inline(always)]
    pub fn query(&self, name: &str) -> Option<&str> {
        self.query_params
            .iter()
            .find(|(k, _)| &**k == name)
            .map(|(_, v)| &**v)
    }

    /// Returns the MIME type (media type portion of Content-Type without parameters/charset).
    #[inline(always)]
    pub fn mime_type(&self) -> Option<String> {
        let ct = self.header("content-type")?;
        let mime = ct.split(';').next()?.trim();
        if mime.is_empty() {
            None
        } else {
            Some(mime.to_lowercase())
        }
    }

    /// Gets the multipart fields
    pub fn get_multipart_fields(&self) -> Result<Vec<MultipartField>, String> {
        let content_type = self
            .header("content-type")
            .ok_or_else(|| "content-type not found".to_string())?;

        let boundary = extract_boundary_from_content_type(content_type)
            .ok_or_else(|| "boundary not found".to_string())?;

        let cursor = Cursor::new(&self.body);
        parse_multipart(cursor, &boundary)
    }
}

#[cfg(test)]
mod tests;
