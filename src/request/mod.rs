use memchr::memchr;

pub struct Request<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub version: &'a str,

    pub headers: Vec<(&'a str, &'a str)>,
    pub params: Vec<(&'a str, &'a str)>,
    pub query_params: Vec<(&'a str, &'a str)>,
}

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

        Some(Self {
            method,
            path,
            version,
            headers,
            params: Vec::with_capacity(4),
            query_params,
        })
    }

    #[inline(always)]
    fn parse_path_and_query<'b>(full_path: &'b str) -> (&'b str, Vec<(&'b str, &'b str)>) {
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
                params.push((&query[start..eq], &query[eq + 1..end]));
            }

            start = end + 1;
        }

        (path, params)
    }

    #[inline(always)]
    fn parse_headers<'b>(lines: &mut std::str::Split<'b, &str>) -> Vec<(&'b str, &'b str)> {
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

            headers.push((key, &line[start..end]));
        }

        headers
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
