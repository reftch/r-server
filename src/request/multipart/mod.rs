use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};

#[derive(Debug, PartialEq, Eq)]
pub struct MultipartField {
    pub name: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub data: Vec<u8>,
}

/// Detects if the Content-Type header indicates a multipart/form-data request
/// and extracts the boundary parameter if present.
pub fn extract_multipart_boundary(headers: &HashMap<String, String>) -> Option<String> {
    let content_type = headers.get("content-type")?;

    if !content_type
        .to_lowercase()
        .starts_with("multipart/form-data")
    {
        return None;
    }

    for param in content_type.split(';') {
        let trimmed = param.trim();
        if trimmed.to_lowercase().starts_with("boundary=") {
            let boundary = &trimmed["boundary=".len()..];
            return Some(boundary.trim_matches('"').to_string());
        }
    }

    None
}

/// Parses a multipart request stream using the specified boundary.
pub fn parse_multipart<R: Read>(reader: R, boundary: &str) -> Result<Vec<MultipartField>, String> {
    let mut buf_reader = BufReader::new(reader);
    let boundary_bytes = format!("--{}", boundary).into_bytes();

    let is_empty_payload = skip_preamble(&mut buf_reader, &boundary_bytes)?;
    if is_empty_payload {
        return Ok(Vec::new());
    }

    let mut fields = Vec::new();
    loop {
        let (field, is_closing) = parse_single_part(&mut buf_reader, &boundary_bytes)?;
        fields.push(field);

        if is_closing {
            break;
        }
    }

    Ok(fields)
}

fn skip_preamble<R: BufRead>(reader: &mut R, boundary_bytes: &[u8]) -> Result<bool, String> {
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?
            == 0
        {
            return Err("Unexpected EOF searching for initial boundary".into());
        }
        let trimmed = trim_newline(&line);
        if trimmed == boundary_bytes {
            return Ok(false);
        }
        if trimmed.starts_with(boundary_bytes) && trimmed.ends_with(b"--") {
            return Ok(true);
        }
    }
}

fn parse_single_part<R: BufRead>(
    reader: &mut R,
    boundary_bytes: &[u8],
) -> Result<(MultipartField, bool), String> {
    let headers = read_part_headers(reader)?;

    let cd = headers
        .get("content-disposition")
        .ok_or_else(|| "Missing Content-Disposition header".to_string())?;

    let name = parse_param(cd, "name")
        .ok_or_else(|| "Missing 'name' in Content-Disposition".to_string())?;
    let filename = parse_param(cd, "filename");
    let content_type = headers.get("content-type").cloned();

    let (data, is_closing) = read_part_body(reader, boundary_bytes)?;

    let field = MultipartField {
        name,
        filename,
        content_type,
        data,
    };

    Ok((field, is_closing))
}

fn read_part_headers<R: BufRead>(reader: &mut R) -> Result<HashMap<String, String>, String> {
    let mut headers = HashMap::new();
    let mut line = Vec::new();

    loop {
        line.clear();
        if reader
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?
            == 0
        {
            return Err("Unexpected EOF reading part headers".into());
        }

        let trimmed = trim_newline(&line);
        if trimmed.is_empty() {
            break;
        }

        let header_str = String::from_utf8_lossy(trimmed);
        if let Some((k, v)) = header_str.split_once(':') {
            headers.insert(k.trim().to_lowercase(), v.trim().to_string());
        }
    }

    Ok(headers)
}

fn read_part_body<R: BufRead>(
    reader: &mut R,
    boundary_bytes: &[u8],
) -> Result<(Vec<u8>, bool), String> {
    let mut data = Vec::new();

    // Check if body is empty (reader points directly at boundary without preceding \r\n)
    if let Ok(buf) = reader.fill_buf() {
        if buf.starts_with(boundary_bytes) {
            reader.consume(boundary_bytes.len());

            let is_closing = if let Ok(next_buf) = reader.fill_buf() {
                next_buf.starts_with(b"--")
            } else {
                false
            };

            let mut dummy = Vec::new();
            let _ = reader.read_until(b'\n', &mut dummy);

            return Ok((Vec::new(), is_closing));
        }
    }

    let mut delimiter = Vec::with_capacity(boundary_bytes.len() + 2);
    delimiter.extend_from_slice(b"\r\n");
    delimiter.extend_from_slice(boundary_bytes);

    let mut matched_len = 0;

    loop {
        let byte = match reader.fill_buf() {
            Ok(buf) if !buf.is_empty() => buf[0],
            Ok(_) => return Err("Unexpected EOF reading part body".into()),
            Err(e) => return Err(e.to_string()),
        };

        if byte == delimiter[matched_len] {
            matched_len += 1;
            reader.consume(1);

            if matched_len == delimiter.len() {
                let is_closing = if let Ok(buf) = reader.fill_buf() {
                    buf.starts_with(b"--")
                } else {
                    false
                };

                let mut dummy = Vec::new();
                let _ = reader.read_until(b'\n', &mut dummy);

                return Ok((data, is_closing));
            }
        } else if matched_len > 0 {
            data.extend_from_slice(&delimiter[..matched_len]);
            matched_len = 0;
        } else {
            data.push(byte);
            reader.consume(1);
        }
    }
}

fn trim_newline(bytes: &[u8]) -> &[u8] {
    let mut end = bytes.len();
    if end > 0 && bytes[end - 1] == b'\n' {
        end -= 1;
    }
    if end > 0 && bytes[end - 1] == b'\r' {
        end -= 1;
    }
    &bytes[..end]
}

fn parse_param(header: &str, key: &str) -> Option<String> {
    let target = format!("{}=", key);
    for part in header.split(';') {
        let trimmed = part.trim();
        if trimmed.starts_with(&target) {
            let value = &trimmed[target.len()..];
            return Some(value.trim_matches('"').to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_multipart_boundary() {
        let mut headers = HashMap::new();
        headers.insert(
            "content-type".to_string(),
            "multipart/form-data; boundary=---------------------------974767299852498929531610575"
                .to_string(),
        );

        let boundary = extract_multipart_boundary(&headers);
        assert_eq!(
            boundary,
            Some("---------------------------974767299852498929531610575".to_string())
        );

        headers.insert("content-type".to_string(), "application/json".to_string());
        assert_eq!(extract_multipart_boundary(&headers), None);
    }

    #[test]
    fn test_parse_simple_multipart() {
        let boundary = "XYZ123";
        let body = format!(
            "--{}\r\n\
             Content-Disposition: form-data; name=\"field1\"\r\n\r\n\
             hello world\r\n\
             --{}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"a.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n\
             file contents\r\n\
             --{}--\r\n",
            boundary, boundary, boundary
        );

        let fields = parse_multipart(body.as_bytes(), boundary).unwrap();
        assert_eq!(fields.len(), 2);

        assert_eq!(fields[0].name, "field1");
        assert_eq!(fields[0].data, b"hello world");
        assert_eq!(fields[0].filename, None);

        assert_eq!(fields[1].name, "file");
        assert_eq!(fields[1].filename.as_deref(), Some("a.txt"));
        assert_eq!(fields[1].content_type.as_deref(), Some("text/plain"));
        assert_eq!(fields[1].data, b"file contents");
    }

    #[test]
    fn test_empty_body() {
        let boundary = "B";
        let body = format!(
            "--{}\r\n\
             Content-Disposition: form-data; name=\"empty\"\r\n\r\n\
             --{}--\r\n",
            boundary, boundary
        );

        let fields = parse_multipart(body.as_bytes(), boundary).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "empty");
        assert_eq!(fields[0].data, b"");
    }

    #[test]
    fn test_completely_empty_stream() {
        let boundary = "EMPTY_STREAM";
        let result = parse_multipart(&b""[..], boundary);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Unexpected EOF searching for initial boundary"
        );
    }

    #[test]
    fn test_empty_multipart_payload() {
        let boundary = "NO_PARTS";
        let body = format!("--{}--\r\n", boundary);

        let fields = parse_multipart(body.as_bytes(), boundary);

        assert!(fields.is_ok());
        assert_eq!(fields.unwrap().len(), 0);
    }

    #[test]
    fn test_binary_data_with_boundary_like_content() {
        let boundary = "abc";
        let body = format!(
            "--{}\r\n\
             Content-Disposition: form-data; name=\"bin\"\r\n\r\n\
             \r\n--abXdata\r\n--{}--\r\n",
            boundary, boundary
        );

        let fields = parse_multipart(body.as_bytes(), boundary).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].data, b"\r\n--abXdata");
    }

    #[test]
    fn test_multiple_fields() {
        let boundary = "MULTI";
        let body = format!(
            "--{}\r\n\
             Content-Disposition: form-data; name=\"a\"\r\n\r\n\
             1\r\n\
             --{}\r\n\
             Content-Disposition: form-data; name=\"b\"\r\n\r\n\
             2\r\n\
             --{}\r\n\
             Content-Disposition: form-data; name=\"c\"\r\n\r\n\
             3\r\n\
             --{}--\r\n",
            boundary, boundary, boundary, boundary
        );

        let fields = parse_multipart(body.as_bytes(), boundary).unwrap();
        assert_eq!(fields.len(), 3);

        assert_eq!(fields[0].name, "a");
        assert_eq!(fields[0].data, b"1");

        assert_eq!(fields[1].name, "b");
        assert_eq!(fields[1].data, b"2");

        assert_eq!(fields[2].name, "c");
        assert_eq!(fields[2].data, b"3");
    }

    #[test]
    fn test_field_without_content_type() {
        let boundary = "NOCT";
        let body = format!(
            "--{}\r\n\
             Content-Disposition: form-data; name=\"noct\"\r\n\r\n\
             data\r\n\
             --{}--\r\n",
            boundary, boundary
        );

        let fields = parse_multipart(body.as_bytes(), boundary).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].content_type, None);
        assert_eq!(fields[0].data, b"data");
    }

    #[test]
    fn test_crlf_in_body() {
        let boundary = "CRLF";
        let body = format!(
            "--{}\r\n\
             Content-Disposition: form-data; name=\"text\"\r\n\r\n\
             line1\r\nline2\r\nline3\r\n\
             --{}--\r\n",
            boundary, boundary
        );

        let fields = parse_multipart(body.as_bytes(), boundary).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].data, b"line1\r\nline2\r\nline3");
    }

    #[test]
    fn test_missing_content_disposition() {
        let boundary = "MISSING";
        let body = format!(
            "--{}\r\n\
             Content-Type: text/plain\r\n\r\n\
             data\r\n\
             --{}--\r\n",
            boundary, boundary
        );

        let result = parse_multipart(body.as_bytes(), boundary);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_name() {
        let boundary = "NONAME";
        let body = format!(
            "--{}\r\n\
             Content-Disposition: form-data\r\n\r\n\
             data\r\n\
             --{}--\r\n",
            boundary, boundary
        );

        let result = parse_multipart(body.as_bytes(), boundary);
        assert!(result.is_err());
    }

    #[test]
    fn test_unexpected_eof() {
        let boundary = "EOF";
        let body = format!(
            "--{}\r\n\
             Content-Disposition: form-data; name=\"x\"\r\n\r\n\
             data",
            boundary
        );

        let result = parse_multipart(body.as_bytes(), boundary);
        assert!(result.is_err());
    }

    #[test]
    fn test_filename_without_name() {
        let boundary = "FNAME";
        let body = format!(
            "--{}\r\n\
             Content-Disposition: form-data; filename=\"test.txt\"\r\n\r\n\
             content\r\n\
             --{}--\r\n",
            boundary, boundary
        );

        let result = parse_multipart(body.as_bytes(), boundary);
        assert!(result.is_err());
    }

    #[test]
    fn test_whitespace_in_headers() {
        let boundary = "WS";
        let body = format!(
            "--{}\r\n\
             Content-Disposition:  form-data;  name=\"wsfield\"  ;  filename=\"ws.txt\"  \r\n\
             Content-Type:  text/plain  \r\n\r\n\
             data\r\n\
             --{}--\r\n",
            boundary, boundary
        );

        let fields = parse_multipart(body.as_bytes(), boundary).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "wsfield");
        assert_eq!(fields[0].filename.as_deref(), Some("ws.txt"));
        assert_eq!(fields[0].content_type.as_deref(), Some("text/plain"));
        assert_eq!(fields[0].data, b"data");
    }

    #[test]
    fn test_large_body() {
        let boundary = "LARGE";
        let data = "x".repeat(1000);
        let body = format!(
            "--{}\r\n\
             Content-Disposition: form-data; name=\"big\"\r\n\r\n\
             {}\r\n\
             --{}--\r\n",
            boundary, data, boundary
        );

        let fields = parse_multipart(body.as_bytes(), boundary).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].data.len(), 1000);
        assert_eq!(&fields[0].data, data.as_bytes());
    }
}
