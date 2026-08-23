use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};

use memchr::memchr;

#[derive(Debug, PartialEq, Eq)]
pub struct FormField {
    pub name: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub data: Vec<u8>,
}

impl FormField {
    /// Field payload decoded as UTF-8 (invalid bytes are replaced).
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.data).to_string()
    }
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

/// Extracts the boundary parameter from a raw Content-Type value.
pub fn extract_boundary_from_content_type(content_type: &str) -> Option<String> {
    let first_part = content_type.split(';').next()?.trim();
    if !first_part.eq_ignore_ascii_case("multipart/form-data") {
        return None;
    }

    for param in content_type.split(';').skip(1) {
        let trimmed = param.trim();
        if let Some((key, val)) = trimmed.split_once('=')
            && key.trim().eq_ignore_ascii_case("boundary")
        {
            let boundary_val = val.trim().trim_matches('"');
            if !boundary_val.is_empty() {
                return Some(boundary_val.to_string());
            }
        }
    }

    None
}

/// Parses a multipart request stream using the specified boundary.
pub fn parse_multipart<R: Read>(reader: R, boundary: &str) -> Result<Vec<FormField>, String> {
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
) -> Result<(FormField, bool), String> {
    let headers = read_part_headers(reader)?;

    let cd = get_header_ignore_case(&headers, "content-disposition")
        .ok_or_else(|| "Missing Content-Disposition header".to_string())?;

    let name = parse_param(cd, "name")
        .ok_or_else(|| "Missing 'name' in Content-Disposition".to_string())?;
    let filename = parse_param(cd, "filename");
    let content_type = get_header_ignore_case(&headers, "content-type").cloned();

    let (data, is_closing) = read_part_body(reader, boundary_bytes)?;

    let field = FormField {
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

    // Tolerate parts whose body is empty and are immediately followed by a
    // boundary line without the usual preceding CRLF (lenient mode).
    {
        let available = reader.fill_buf().map_err(|e| e.to_string())?;
        if bare_boundary_follows(available, boundary_bytes) {
            reader.consume(boundary_bytes.len());
            return Ok((data, read_boundary_suffix(reader)?));
        }
    }

    // Number of leading delimiter bytes currently matched. Bytes matched so
    // far are held back from `data` until the match either completes or fails,
    // which keeps matching correct across buffer boundaries of any size.
    let mut matched = 0usize;

    loop {
        let available = match reader.fill_buf() {
            Ok(buf) if !buf.is_empty() => buf,
            Ok(_) => return Err("Unexpected EOF reading part body".into()),
            Err(e) => return Err(e.to_string()),
        };

        let mut scanned = 0usize;
        let mut boundary_hit = false;

        while scanned < available.len() {
            // Fast path: while nothing is pending, bulk-emit up to the next
            // '\r', the only byte that can start the delimiter.
            if matched == 0 {
                match memchr(b'\r', &available[scanned..]) {
                    Some(rel) => {
                        data.extend_from_slice(&available[scanned..scanned + rel]);
                        scanned += rel + 1;
                        matched = 1;
                    }
                    None => {
                        data.extend_from_slice(&available[scanned..]);
                        scanned = available.len();
                    }
                }
                continue;
            }

            let byte = available[scanned];
            scanned += 1;

            if byte == delimiter[matched] {
                matched += 1;
                if matched == delimiter.len() {
                    boundary_hit = true;
                    break;
                }
            } else {
                // Mismatch: flush the held-back prefix into the payload, then
                // emit (or re-hold) the current byte.
                data.extend_from_slice(&delimiter[..matched]);
                if byte == delimiter[0] {
                    matched = 1;
                } else {
                    matched = 0;
                    data.push(byte);
                }
            }
        }

        reader.consume(scanned);

        if boundary_hit {
            return Ok((data, read_boundary_suffix(reader)?));
        }
    }
}

/// Checks whether the buffered slice begins a bare boundary line
/// ("--boundary", optionally followed by "--") without a preceding CRLF.
fn bare_boundary_follows(available: &[u8], boundary_bytes: &[u8]) -> bool {
    if !available.starts_with(boundary_bytes) {
        return false;
    }

    match available.get(boundary_bytes.len()) {
        None => true,
        Some(b'-' | b'\r' | b'\n') => true,
        Some(_) => false,
    }
}

/// Reads the remainder of a boundary line ("--" for the closing marker) and
/// reports whether the part stream is finished.
fn read_boundary_suffix<R: BufRead>(reader: &mut R) -> Result<bool, String> {
    let mut suffix = Vec::new();
    reader
        .read_until(b'\n', &mut suffix)
        .map_err(|e| e.to_string())?;
    Ok(trim_newline(&suffix).starts_with(b"--"))
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
        if let Some((k, v)) = trimmed.split_once('=')
            && k.trim().eq_ignore_ascii_case(key)
        {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_boundary_from_content_type() {
        let boundary = extract_boundary_from_content_type(
            "multipart/form-data; boundary=---------------------------974767299852498929531610575",
        );
        assert_eq!(
            boundary,
            Some("---------------------------974767299852498929531610575".to_string())
        );

        assert_eq!(extract_boundary_from_content_type("application/json"), None);
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

    /// Deterministic pseudo-random byte generator (xorshift64*), so tests
    /// exercise realistic binary payloads without external dependencies.
    fn pseudo_random_bytes(seed: u64, len: usize) -> Vec<u8> {
        let mut state = seed;
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 32) as u8
            })
            .collect()
    }

    /// Wraps a payload in a standard browser-style multipart body with a
    /// single file field.
    fn build_file_upload(
        boundary: &str,
        filename: &str,
        content_type: &str,
        data: &[u8],
    ) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
                filename
            )
            .as_bytes(),
        );
        body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", content_type).as_bytes());
        body.extend_from_slice(data);
        body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());
        body
    }

    struct ChunkedReader {
        data: Vec<u8>,
        pos: usize,
        chunk: usize,
    }

    impl Read for ChunkedReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.pos >= self.data.len() {
                return Ok(0);
            }
            let end = (self.pos + self.chunk).min(self.data.len());
            let n = end - self.pos;
            buf[..n].copy_from_slice(&self.data[self.pos..end]);
            self.pos = end;
            Ok(n)
        }
    }

    #[test]
    fn test_binary_png_like_file_roundtrip() {
        // PNG magic number followed by binary chunks including \r, \n and \0
        let mut png = Vec::new();
        png.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        png.extend_from_slice(&[0x00, 0x00, 0x00, 0x0D]);
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&pseudo_random_bytes(42, 512));

        let boundary = "----WebKitFormBoundaryABC123";
        let body = build_file_upload(boundary, "image.png", "image/png", &png);

        let fields = parse_multipart(body.as_slice(), boundary).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "file");
        assert_eq!(fields[0].filename.as_deref(), Some("image.png"));
        assert_eq!(fields[0].content_type.as_deref(), Some("image/png"));
        assert_eq!(fields[0].data, png);
    }

    #[test]
    fn test_binary_file_ending_with_cr() {
        // Payload ending in '\r' directly before the terminating \r\n boundary
        let mut data = pseudo_random_bytes(7, 300);
        data.push(b'\r');

        let boundary = "BOUND";
        let body = build_file_upload(boundary, "blob.bin", "application/octet-stream", &data);

        let fields = parse_multipart(body.as_slice(), boundary).unwrap();
        assert_eq!(fields[0].data, data);
    }

    #[test]
    fn test_binary_containing_boundary_prefix_sequences() {
        // Payload full of near-boundary sequences that must NOT terminate it
        let boundary = "----WebKitFormBoundaryXYZ";
        let delimiter = format!("\r\n{}", boundary);

        let delimiter_bytes = delimiter.as_bytes();

        let mut data = Vec::new();
        data.extend_from_slice(b"start\r\n--");
        data.extend_from_slice(&delimiter_bytes[..delimiter_bytes.len() - 1]); // missing last char
        data.extend_from_slice(b"\r\n\r\n--");
        data.push(b'-'); // one extra dash
        data.extend_from_slice(delimiter_bytes);
        data.pop(); // drop final char so it is only a prefix
        data.extend_from_slice(b"\r\nend");

        let body = build_file_upload(boundary, "tricky.bin", "application/octet-stream", &data);

        for chunk in [1usize, 2, 3, 5, 8, 64] {
            let reader = ChunkedReader {
                data: body.clone(),
                pos: 0,
                chunk,
            };
            let fields = parse_multipart(reader, boundary)
                .unwrap_or_else(|e| panic!("chunk {} failed: {}", chunk, e));
            assert_eq!(fields.len(), 1, "chunk {}", chunk);
            assert_eq!(
                fields[0].data, data,
                "payload corrupted with chunk size {}",
                chunk
            );
        }
    }

    #[test]
    fn test_chunked_delivery_all_sizes_roundtrip() {
        // Deliver the same upload with every chunk size from 1 to 70 bytes to
        // force the delimiter to be split across buffer boundaries everywhere.
        let boundary = "----WebKitFormBoundaryChunked";
        let file_data = pseudo_random_bytes(0xDEAD_BEEF, 4000);
        let body = build_file_upload(boundary, "blob.bin", "application/octet-stream", &file_data);

        for chunk in 1..=70usize {
            let reader = ChunkedReader {
                data: body.clone(),
                pos: 0,
                chunk,
            };
            let fields = parse_multipart(reader, boundary)
                .unwrap_or_else(|e| panic!("chunk size {} failed: {}", chunk, e));
            assert_eq!(fields.len(), 1, "chunk size {}", chunk);
            assert_eq!(
                fields[0].data, file_data,
                "binary corruption with chunk size {}",
                chunk
            );
        }
    }

    #[test]
    fn test_large_binary_crossing_bufreader_capacity() {
        // Larger than BufReader's default 8 KiB capacity
        let file_data = pseudo_random_bytes(99, 40 * 1024 + 137);
        let boundary = "----WebKitFormBoundaryLarge";

        let fields = parse_multipart(
            build_file_upload(boundary, "big.bin", "application/zip", &file_data).as_slice(),
            boundary,
        )
        .unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].data, file_data);
    }

    #[test]
    fn test_mixed_text_and_multiple_binary_files() {
        let img = pseudo_random_bytes(1, 1000);
        let zip = pseudo_random_bytes(2, 2000);
        let boundary = "MIXED123";

        let mut body = Vec::new();

        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"title\"\r\n\r\n");
        body.extend_from_slice(b"My Vacation\r\n");

        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"photo\"; filename=\"a.png\"\r\n",
        );
        body.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
        body.extend_from_slice(&img);
        body.extend_from_slice(b"\r\n");

        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"archive\"; filename=\"b.zip\"\r\n",
        );
        body.extend_from_slice(b"Content-Type: application/zip\r\n\r\n");
        body.extend_from_slice(&zip);
        body.extend_from_slice(b"\r\n");

        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let fields = parse_multipart(body.as_slice(), boundary).unwrap();
        assert_eq!(fields.len(), 3);

        assert_eq!(fields[0].name, "title");
        assert_eq!(fields[0].filename, None);
        assert_eq!(fields[0].data, b"My Vacation");

        assert_eq!(fields[1].name, "photo");
        assert_eq!(fields[1].filename.as_deref(), Some("a.png"));
        assert_eq!(fields[1].content_type.as_deref(), Some("image/png"));
        assert_eq!(fields[1].data, img);

        assert_eq!(fields[2].name, "archive");
        assert_eq!(fields[2].filename.as_deref(), Some("b.zip"));
        assert_eq!(fields[2].data, zip);
    }

    #[test]
    fn test_all_byte_values_roundtrip() {
        // Every possible byte value repeated, including all CRLF variants
        let mut data = Vec::with_capacity(256 * 4);
        for b in 0..=255u8 {
            data.extend_from_slice(&[b, b'\r', b'\n', b]);
        }

        let boundary = "ALLBYTES";
        let body = build_file_upload(boundary, "all.bin", "application/octet-stream", &data);

        let fields = parse_multipart(body.as_slice(), boundary).unwrap();
        assert_eq!(fields[0].data, data);
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
