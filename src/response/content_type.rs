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
}
