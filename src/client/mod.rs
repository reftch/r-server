use std::io::{Read, Write}; // Required for write! macro

/// A trait that combines `Read` and `Write` capabilities.
pub trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}

/// A simple HTTP client capable of making GET, POST, PUT, PATCH, and DELETE requests.
/// It supports both HTTP and HTTPS via SSL/TLS.
pub struct Client {
    host: String,
    is_secure: bool,
    port: u16,
}

impl Client {
    /// Creates a new `Client` instance.
    ///
    /// The `host` parameter can include a scheme like `http://` or `https://`.
    /// If a scheme is provided, it is stripped, and the `is_secure` flag and
    /// appropriate port (443 for https, 80 for http) are set accordingly.
    pub fn new(host: impl Into<String>) -> Self {
        let host_str = host.into();
        let is_secure = host_str.starts_with("https://");
        let default_port = if is_secure { 443 } else { 80 };

        let stripped = if is_secure {
            host_str
                .strip_prefix("https://")
                .unwrap_or(&host_str)
                .to_string()
        } else if host_str.starts_with("http://") {
            host_str
                .strip_prefix("http://")
                .unwrap_or(&host_str)
                .to_string()
        } else {
            host_str
        };

        // Only the authority (host[:port]) is used to open the connection; any
        // path or query component in the input is ignored here and must be
        // supplied per-request via the `path` argument.
        let authority = match stripped.find('/') {
            Some(i) => &stripped[..i],
            None => &stripped[..],
        };

        let (cleaned_host, port) = split_authority(authority, default_port);

        Self {
            host: cleaned_host,
            is_secure,
            port,
        }
    }

    /// Executes an HTTP request with the given method, path, optional body,
    /// and extra headers.
    ///
    /// This method handles stream creation, request construction, and response reading,
    /// including support for chunked transfer encoding.
    fn execute(
        &self,
        method: &str,
        path: &str,
        body: Option<String>,
        headers: &[(&str, &str)],
    ) -> Result<String, String> {
        // Use the custom ReadWrite trait for the Box type
        let mut stream: Box<dyn ReadWrite> = if self.is_secure {
            let connector = openssl::ssl::SslConnector::builder(openssl::ssl::SslMethod::tls())
                .map_err(|e| e.to_string())?
                .build();

            // Use .as_str() to pass &str instead of &String to satisfy ToSocketAddrs
            let tcp = std::net::TcpStream::connect((self.host.as_str(), self.port))
                .map_err(|e| e.to_string())?;

            let ssl_stream = connector
                .connect(self.host.as_str(), tcp)
                .map_err(|e| e.to_string())?;

            // This now works because ssl_stream implements Read + Write
            Box::new(ssl_stream)
        } else {
            // Again, use .as_str() here
            let tcp = std::net::TcpStream::connect((self.host.as_str(), self.port))
                .map_err(|e| e.to_string())?;

            Box::new(tcp)
        };

        // Now write! will work because Write is in scope
        write!(stream, "{} {} HTTP/1.1\r\n", method, path).map_err(|e| e.to_string())?;

        // Include the port in the Host header unless it is the scheme's default.
        // Some servers (e.g. Keycloak) validate the Host against the issuer URL
        // and reject requests where the port is omitted for non-default ports.
        let host_header =
            if (self.is_secure && self.port == 443) || (!self.is_secure && self.port == 80) {
                self.host.clone()
            } else {
                format!("{}:{}", self.host, self.port)
            };
        write!(stream, "Host: {}\r\n", host_header).map_err(|e| e.to_string())?;
        write!(stream, "User-Agent: r-server\r\n").map_err(|e| e.to_string())?;
        write!(stream, "Accept: application/json\r\n").map_err(|e| e.to_string())?;
        write!(stream, "Connection: close\r\n").map_err(|e| e.to_string())?;

        for (name, value) in headers {
            write!(stream, "{}: {}\r\n", name, value).map_err(|e| e.to_string())?;
        }

        if let Some(ref b) = body {
            write!(stream, "Content-Length: {}\r\n", b.len()).map_err(|e| e.to_string())?;
        }

        write!(stream, "\r\n").map_err(|e| e.to_string())?;

        if let Some(b) = body {
            stream.write_all(b.as_bytes()).map_err(|e| e.to_string())?;
        }
        stream.flush().map_err(|e| e.to_string())?;

        let mut raw_response = Vec::new();
        stream
            .read_to_end(&mut raw_response)
            .map_err(|e| e.to_string())?;

        let header_end = raw_response
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or_else(|| "Invalid HTTP response: headers not found".to_string())?;

        let headers = &raw_response[..header_end];
        let body_start = header_end + 4;
        let body_bytes = &raw_response[body_start..];

        let headers_str = String::from_utf8_lossy(headers);
        let is_chunked = headers_str
            .to_lowercase()
            .contains("transfer-encoding: chunked");

        let final_body_bytes = if is_chunked {
            decode_chunked(body_bytes).map_err(|e| e.to_string())?
        } else {
            body_bytes.to_vec()
        };

        String::from_utf8(final_body_bytes).map_err(|e| e.to_string())
    }

    /// Sends a GET request to the specified path.
    pub fn get(&self, path: &str) -> Result<String, String> {
        self.get_with(path, &[])
    }

    /// Sends a GET request to the specified path with extra headers.
    pub fn get_with(&self, path: &str, headers: &[(&str, &str)]) -> Result<String, String> {
        self.execute("GET", path, None, headers)
    }

    /// Sends a POST request to the specified path with the given body.
    pub fn post(&self, path: &str, body: String) -> Result<String, String> {
        self.post_with(path, body, &[])
    }

    /// Sends a POST request to the specified path with the given body and extra headers.
    pub fn post_with(
        &self,
        path: &str,
        body: String,
        headers: &[(&str, &str)],
    ) -> Result<String, String> {
        self.execute("POST", path, Some(body), headers)
    }

    /// Sends a PUT request to the specified path with the given body.
    pub fn put(&self, path: &str, body: String) -> Result<String, String> {
        self.execute("PUT", path, Some(body), &[])
    }

    /// Sends a PATCH request to the specified path with the given body.
    pub fn patch(&self, path: &str, body: String) -> Result<String, String> {
        self.execute("PATCH", path, Some(body), &[])
    }

    /// Sends a DELETE request to the specified path.
    pub fn delete(&self, path: &str) -> Result<String, String> {
        self.execute("DELETE", path, None, &[])
    }
}

/// Decodes a body that has been encoded using chunked transfer encoding.
pub fn decode_chunked(body: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut result = Vec::new();
    let mut pos = 0;

    loop {
        let end = body[pos..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("Invalid chunked encoding")?;

        let size_str = std::str::from_utf8(&body[pos..pos + end])?;
        let size = usize::from_str_radix(size_str.trim(), 16)?;

        pos += end + 2;

        if size == 0 {
            break;
        }

        if pos + size > body.len() {
            return Err("Incomplete chunk".into());
        }

        result.extend_from_slice(&body[pos..pos + size]);
        pos += size;

        if body.get(pos..pos + 2) != Some(b"\r\n") {
            return Err("Invalid chunk terminator".into());
        }
        pos += 2;
    }
    Ok(result)
}

/// Splits an authority string into a host and port.
///
/// Supports `host`, `host:port`, and IPv6 literals such as `[::1]` or
/// `[::1]:9090`. When no port is present, `default_port` is returned.
fn split_authority(authority: &str, default_port: u16) -> (String, u16) {
    // IPv6 literal, e.g. "[::1]" or "[::1]:9090"
    if let Some(rest) = authority.strip_prefix('[') {
        if let Some(close) = rest.find(']') {
            let host = format!("[{}]", &rest[..close]);
            let after = &rest[close + 1..];
            if let Some(port_str) = after.strip_prefix(':') {
                if let Ok(port) = port_str.parse::<u16>() {
                    return (host, port);
                }
            }
            return (host, default_port);
        }
    }

    // host:port (IPv4 or hostname)
    if let Some(colon) = authority.rfind(':') {
        let (host, port_part) = authority.split_at(colon);
        if let Ok(port) = port_part[1..].parse::<u16>() {
            return (host.to_string(), port);
        }
    }

    (authority.to_string(), default_port)
}

#[cfg(test)]
mod tests;
