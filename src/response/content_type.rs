use std::path::Path;

#[derive(Debug, PartialEq)]
pub enum ContentType {
    HTML,
    CSS,
    JAVASCRIPT,
    JPEG,
    PNG,
    XML,
    JSON,
    TEXT,
    GIF,
    SVG,
    PDF,
    MP3,
    MP4,
    WEBM,
    WOFF2,
    TTF,
    EOT,
    SSE, // Server-Sent Events
    UNKNOWN,
}

impl ContentType {
    /// Returns the standard MIME type string for the content type.
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentType::HTML => "text/html",
            ContentType::CSS => "text/css",
            ContentType::JAVASCRIPT => "text/javascript",
            ContentType::JPEG => "image/jpeg",
            ContentType::PNG => "image/png",
            ContentType::XML => "application/xml",
            ContentType::JSON => "application/json",
            ContentType::TEXT => "text/plain",
            ContentType::GIF => "image/gif",
            ContentType::SVG => "image/svg+xml",
            ContentType::PDF => "application/pdf",
            ContentType::MP3 => "audio/mpeg",
            ContentType::MP4 => "video/mp4",
            ContentType::WEBM => "video/webm",
            ContentType::WOFF2 => "font/woff2",
            ContentType::TTF => "font/ttf",
            ContentType::EOT => "application/vnd.ms-fontobject",
            ContentType::SSE => "text/event-stream",
            ContentType::UNKNOWN => "application/octet-stream",
        }
    }

    pub fn get_content_type(path: &Path) -> ContentType {
        match path.extension().and_then(|s| s.to_str()) {
            Some("html") => ContentType::HTML,
            Some("css") => ContentType::CSS,
            Some("js") => ContentType::JAVASCRIPT,
            Some("jpg") | Some("jpeg") => ContentType::JPEG,
            Some("png") => ContentType::PNG,
            Some("xml") => ContentType::XML,
            Some("json") => ContentType::JSON,
            Some("txt") => ContentType::TEXT,
            Some("gif") => ContentType::GIF,
            Some("svg") => ContentType::SVG,
            Some("pdf") => ContentType::PDF,
            Some("mp3") => ContentType::MP3,
            Some("mp4") => ContentType::MP4,
            Some("webm") => ContentType::WEBM,
            Some("woff2") => ContentType::WOFF2,
            Some("ttf") => ContentType::TTF,
            _ => ContentType::UNKNOWN,
        }
    }
}
