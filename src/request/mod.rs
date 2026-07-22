// use std::collections::HashMap;

// use crate::trace;

// pub struct Request<'a> {
//     pub method: &'a str,
//     pub path: &'a str,
//     pub headers: HashMap<&'a str, &'a str>,
//     pub params: HashMap<&'a str, &'a str>,
//     pub query_params: HashMap<&'a str, &'a str>,
// }

pub struct Request<'a> {
    pub method: &'a str,
    pub path: &'a str,

    pub headers: Vec<(&'a str, &'a str)>,
    pub params: Vec<(&'a str, &'a str)>,
    pub query_params: Vec<(&'a str, &'a str)>,
}

// impl<'a> Request<'a> {
//     #[inline]
//     fn find_header_end(buf: &[u8]) -> Option<usize> {
//         let mut i = 0;

//         while i + 3 < buf.len() {
//             if buf[i] == b'\r' && buf[i + 1] == b'\n' && buf[i + 2] == b'\r' && buf[i + 3] == b'\n'
//             {
//                 return Some(i + 4);
//             }

//             i += 1;
//         }

//         None
//     }

//     /// Parses a HTTP request from a byte buffer.
//     ///
//     /// # Examples
//     ///
//     /// ```
//     /// use r_server::request::Request;
//     /// let buf = b"GET / HTTP/1.1\r\nContent-Type: application/json\r\n\r\n";
//     /// let request = Request::parse(buf).unwrap();
//     /// assert_eq!(request.method, "GET");
//     /// assert_eq!(request.path, "/");
//     /// ```
//     pub fn parse(buf: &'a [u8]) -> Option<Self> {
//         let header_end = Self::find_header_end(buf)?;

//         let text = std::str::from_utf8(&buf[..header_end]).ok()?;

//         let mut lines = text.split_terminator("\r\n");

//         // Request line
//         let request_line = lines.next()?;

//         let first_space = request_line.find(' ')?;
//         let second_space = {
//             let i = request_line[first_space + 1..].find(' ')?;
//             i + first_space + 1
//         };

//         if second_space >= request_line.len().saturating_sub(1) {
//             return None;
//         }

//         let first_space = request_line.find(' ')?;
//         let second_space = {
//             let i = request_line[first_space + 1..].find(' ')?;
//             i + first_space + 1
//         };

//         if second_space >= request_line.len().saturating_sub(1) {
//             return None;
//         }

//         let method = &request_line[..first_space];
//         let full_path = &request_line[first_space + 1..second_space];

//         let (path, query_params) = match full_path.find('?') {
//             Some(pos) => {
//                 let path = &full_path[..pos];
//                 let mut query_params = HashMap::new();
//                 let query_str = &full_path[pos + 1..];
//                 for pair in query_str.split('&') {
//                     if let Some((key, value)) = pair.split_once('=') {
//                         trace!("Found query parameter [{}={}]", key, value);
//                         query_params.insert(key, value);
//                     }
//                 }
//                 (path, query_params)
//             }
//             None => (full_path, HashMap::new()),
//         };

//         if method.is_empty() || path.is_empty() {
//             return None;
//         }

//         // Headers
//         let mut headers = HashMap::with_capacity(12);

//         for line in lines {
//             let Some((key, value)) = line.split_once(':') else {
//                 continue;
//             };

//             headers.insert(key.trim(), value.trim());
//         }

//         Some(Self {
//             method,
//             path,
//             headers,
//             params: HashMap::with_capacity(4),
//             query_params,
//         })
//     }

//     #[inline]
//     /// Returns the HTTP `Content-Type` header value if present.
//     ///
//     /// # Examples
//     ///
//     /// ```
//     /// use r_server::request::Request;
//     /// let buf = b"POST / HTTP/1.1\r\nContent-Type: application/json\r\n\r\n";
//     /// let request = Request::parse(buf).unwrap();
//     /// assert_eq!(request.mime_type(), Some("application/json"));
//     /// ```
//     pub fn mime_type(&self) -> Option<&str> {
//         self.headers.get("Content-Type").copied()
//     }
// }

impl<'a> Request<'a> {
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

    #[inline(always)]
    pub fn parse(buf: &'a [u8]) -> Option<Self> {
        let header_end = Self::find_header_end(buf)?;

        // HTTP is ASCII. Avoid UTF-8 validation.
        let text = unsafe { std::str::from_utf8_unchecked(&buf[..header_end]) };

        let mut lines = text.split("\r\n");

        // ------------------------
        // Request line
        // ------------------------

        let request_line = lines.next()?;

        let bytes = request_line.as_bytes();

        let mut first_space = None;
        let mut second_space = None;

        for i in 0..bytes.len() {
            if bytes[i] == b' ' {
                if first_space.is_none() {
                    first_space = Some(i);
                } else {
                    second_space = Some(i);
                    break;
                }
            }
        }

        let first_space = first_space?;
        let second_space = second_space?;

        let method = &request_line[..first_space];
        let full_path = &request_line[first_space + 1..second_space];
        if method.is_empty() || full_path.is_empty() {
            return None;
        }

        // ------------------------
        // Query parameters
        // ------------------------

        let mut query_params = Vec::with_capacity(4);

        let path = match full_path.find('?') {
            Some(pos) => {
                let path = &full_path[..pos];
                let query = &full_path[pos + 1..];

                for pair in query.split('&') {
                    if let Some((key, value)) = pair.split_once('=') {
                        query_params.push((key, value));
                    }
                }
                path
            }

            None => full_path,
        };

        // ------------------------
        // Headers
        // ------------------------

        // let mut headers = HashMap::with_capacity(12);
        let mut headers = Vec::with_capacity(12);

        for line in lines {
            if line.is_empty() {
                break;
            }

            if let Some((key, value)) = line.split_once(':') {
                headers.push((key.trim(), value.trim()));
            }
        }

        Some(Self {
            method,
            path,
            headers,
            params: Vec::with_capacity(4),
            // params: HashMap::with_capacity(4),
            query_params,
        })
    }

    #[inline(always)]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.iter().find_map(
            |(key, value)| {
                if *key == name { Some(*value) } else { None }
            },
        )
    }

    #[inline(always)]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find_map(
            |(key, value)| {
                if *key == name { Some(*value) } else { None }
            },
        )
    }

    #[inline(always)]
    pub fn query(&self, name: &str) -> Option<&str> {
        self.query_params.iter().find_map(
            |(key, value)| {
                if *key == name { Some(*value) } else { None }
            },
        )
    }

    #[inline(always)]
    pub fn mime_type(&self) -> Option<&str> {
        self.header("Content-Type")
    }
}

#[cfg(test)]
mod tests;
