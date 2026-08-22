use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};

#[derive(Debug, PartialEq, Eq)]
pub struct MultipartField {
    pub name: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub data: Vec<u8>,
}

/// Case-insensitive header lookup helper that never mutates the underlying map.
fn get_header_ignore_case<'a>(
    headers: &'a HashMap<String, String>,
    key: &str,
) -> Option<&'a String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v)
}

/// Detects if the Content-Type header indicates a multipart/form-data request
/// and extracts the boundary parameter if present.
pub fn extract_multipart_boundary(headers: &HashMap<String, String>) -> Option<String> {
    let content_type = get_header_ignore_case(headers, "content-type")?;

    let first_part = content_type.split(';').next()?.trim();
    if !first_part.eq_ignore_ascii_case("multipart/form-data") {
        return None;
    }

    for param in content_type.split(';').skip(1) {
        let trimmed = param.trim();
        if let Some((key, val)) = trimmed.split_once('=') {
            if key.trim().eq_ignore_ascii_case("boundary") {
                let boundary_val = val.trim().trim_matches('"');
                if !boundary_val.is_empty() {
                    return Some(boundary_val.to_string());
                }
            }
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
        let bytes_read = reader
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;

        if bytes_read == 0 {
            return Err("Unexpected EOF searching for initial boundary".into());
        }

        let trimmed = trim_newline(&line);
        if trimmed.is_empty() {
            continue;
        }

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

    let cd = get_header_ignore_case(&headers, "content-disposition")
        .ok_or_else(|| "Missing Content-Disposition header".to_string())?;

    let name = parse_param(cd, "name")
        .ok_or_else(|| "Missing 'name' in Content-Disposition".to_string())?;
    let filename = parse_param(cd, "filename");
    let content_type = get_header_ignore_case(&headers, "content-type").cloned();

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

    // The boundary in body content is prefixed by \r\n
    let mut delimiter = Vec::with_capacity(boundary_bytes.len() + 2);
    delimiter.extend_from_slice(b"\r\n");
    delimiter.extend_from_slice(boundary_bytes);

    loop {
        let available = match reader.fill_buf() {
            Ok(buf) if !buf.is_empty() => buf,
            Ok(_) => return Err("Unexpected EOF reading part body".into()),
            Err(e) => return Err(e.to_string()),
        };

        // Standard case: full delimiter is visible in current buffer slice
        if available.len() >= delimiter.len() {
            if available.starts_with(&delimiter) {
                reader.consume(delimiter.len());

                let mut suffix = Vec::new();
                let _ = reader.read_until(b'\n', &mut suffix);
                let trimmed = trim_newline(&suffix);
                let is_closing = trimmed.starts_with(b"--");

                return Ok((data, is_closing));
            }
        }

        // Search for possible delimiter start (\r)
        if let Some(pos) = available.iter().position(|&b| b == b'\r') {
            if pos > 0 {
                // Safely stream everything up to the first candidate \r
                data.extend_from_slice(&available[..pos]);
                reader.consume(pos);
            } else {
                // \r is at position 0. Check how much of delimiter we can match in current buffer
                let match_len = available.len().min(delimiter.len());
                if delimiter.starts_with(&available[..match_len]) {
                    if available.len() >= delimiter.len() {
                        // Full match checked and failed above, so move 1 byte forward
                        data.push(available[0]);
                        reader.consume(1);
                    } else {
                        // Partial match at end of current buffer.
                        // Read 1 byte to force buffer fill without corrupting position state
                        let byte = available[0];
                        reader.consume(1);

                        // Peeking next byte after consume
                        let next_buf = match reader.fill_buf() {
                            Ok(buf) => buf,
                            Err(e) => return Err(e.to_string()),
                        };

                        // Reconstruct lookahead
                        let mut check = Vec::with_capacity(1 + next_buf.len());
                        check.push(byte);
                        check.extend_from_slice(next_buf);

                        if check.starts_with(&delimiter) {
                            // Boundary hit! Consume remaining delimiter bytes
                            reader.consume(delimiter.len() - 1);

                            let mut suffix = Vec::new();
                            let _ = reader.read_until(b'\n', &mut suffix);
                            let trimmed = trim_newline(&suffix);
                            let is_closing = trimmed.starts_with(b"--");

                            return Ok((data, is_closing));
                        }

                        // Not a boundary match: push byte to body data and continue
                        data.push(byte);
                    }
                } else {
                    // \r was present but did not match delimiter prefix
                    data.push(available[0]);
                    reader.consume(1);
                }
            }
        } else {
            // No \r found in buffer at all; safely consume whole buffer
            let len = available.len();
            data.extend_from_slice(available);
            reader.consume(len);
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
    for part in header.split(';') {
        let trimmed = part.trim();
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim().eq_ignore_ascii_case(key) {
                return Some(v.trim().trim_matches('"').to_string());
            }
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
            "Content-Type".to_string(),
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
    fn test_binary_data_with_newline_bytes() {
        let boundary = "BIN";
        let body = format!(
            "--{}\r\n\
             Content-Disposition: form-data; name=\"pdf\"; filename=\"test.pdf\"\r\n\
             Content-Type: application/pdf\r\n\r\n\
             %PDF-1.4\n<binary>\n0x0A\nmore\x00\x01\x02\r\n--{}--\r\n",
            boundary, boundary
        );

        let fields = parse_multipart(body.as_bytes(), boundary).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "pdf");
        assert_eq!(fields[0].filename.as_deref(), Some("test.pdf"));
        assert_eq!(fields[0].content_type.as_deref(), Some("application/pdf"));
        assert!(fields[0].data.contains(&0x0A));
        assert!(fields[0].data.contains(&b'%'));
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
