use std::collections::HashMap;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Status {
    // Information responses
    Continue = 100,
    SwitchingProtocols = 101,
    Processing = 102,
    EarlyHints = 103,

    // Successful responses
    Ok = 200,
    Created = 201,
    Accepted = 202,
    NonAuthoritativeInformation = 203,
    NoContent = 204,
    ResetContent = 205,
    PartialContent = 206,
    MultiStatus = 207,
    AlreadyReported = 208,

    // Redirection messages
    MultipleChoices = 300,
    MovedPermanently = 301,
    MovedTemporarily = 302,
    NotModified = 304,
    UseProxy = 305,
    Unused = 306,
    TemporaryRedirect = 307,
    PermanentRedirect = 308,

    // Client error responses
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    NotAcceptable = 406,
    ProxyAuthenticationRequired = 407,
    RequestTimeout = 408,
    Conflict = 409,
    Gone = 410,
    LengthRequired = 411,
    PreconditionFailed = 412,
    PayloadTooLarge = 413,
    UriTooLong = 414,
    UnsupportedMediaType = 415,
    RangeNotSatisfiable = 416,
    ExpectationFailed = 417,
    ImATeapot = 418,
    MisdirectedRequest = 421,
    UnprocessableContent = 422,
    Locked = 423,
    FailedDependency = 424,
    TooEarly = 425,
    UpgradeRequired = 426,
    PreconditionRequired = 428,
    TooManyRequests = 429,
    RequestHeaderFieldsTooLarge = 431,
    UnavailableForLegalReasons = 451,

    // Server error responses
    InternalServerError = 500,
    NotImplemented = 501,
    BadGateway = 502,
    ServiceUnavailable = 503,
    GatewayTimeout = 504,
    HttpVersionNotSupported = 505,
    VariantAlsoNegotiates = 506,
    InsufficientStorage = 507,
    LoopDetected = 508,
    NotExtended = 510,
    NetworkAuthenticationRequired = 511,
}

impl Status {
    pub fn as_u16(&self) -> u16 {
        *self as u16
    }

    pub fn reason_phrase(&self) -> &'static str {
        match self {
            Status::Continue => "Continue",
            Status::SwitchingProtocols => "Switching Protocols",
            Status::Processing => "Processing",
            Status::EarlyHints => "Early Hints",
            Status::Ok => "OK",
            Status::Created => "Created",
            Status::Accepted => "Accepted",
            Status::NonAuthoritativeInformation => "Non-Authoritative Information",
            Status::NoContent => "No Content",
            Status::ResetContent => "Reset Content",
            Status::PartialContent => "Partial Content",
            Status::MultiStatus => "Multi-Status",
            Status::AlreadyReported => "Already Reported",
            Status::MultipleChoices => "Multiple Choices",
            Status::MovedPermanently => "Moved Permanently",
            Status::MovedTemporarily => "Moved Temporarily",
            Status::NotModified => "Not Modified",
            Status::UseProxy => "Use Proxy",
            Status::Unused => "Unused",
            Status::TemporaryRedirect => "Temporary Redirect",
            Status::PermanentRedirect => "Permanent Redirect",
            Status::BadRequest => "Bad Request",
            Status::Unauthorized => "Unauthorized",
            Status::Forbidden => "Forbidden",
            Status::NotFound => "Not Found",
            Status::MethodNotAllowed => "Method Not Allowed",
            Status::NotAcceptable => "Not Acceptable",
            Status::ProxyAuthenticationRequired => "Proxy Authentication Required",
            Status::RequestTimeout => "Request Timeout",
            Status::Conflict => "Conflict",
            Status::Gone => "Gone",
            Status::LengthRequired => "Length Required",
            Status::PreconditionFailed => "Precondition Failed",
            Status::PayloadTooLarge => "Payload Too Large",
            Status::UriTooLong => "URI Too Long",
            Status::UnsupportedMediaType => "Unsupported Media Type",
            Status::RangeNotSatisfiable => "Range Not Satisfiable",
            Status::ExpectationFailed => "Expectation Failed",
            Status::ImATeapot => "Im A Teapot",
            Status::MisdirectedRequest => "Misdirected Request",
            Status::UnprocessableContent => "Unprocessable Content",
            Status::Locked => "Locked",
            Status::FailedDependency => "Failed Dependency",
            Status::TooEarly => "Too Early",
            Status::UpgradeRequired => "Upgrade Required",
            Status::PreconditionRequired => "Precondition Required",
            Status::TooManyRequests => "Too Many Requests",
            Status::RequestHeaderFieldsTooLarge => "Request Header Fields Too Large",
            Status::UnavailableForLegalReasons => "Unavailable For Legal Reasons",
            Status::InternalServerError => "Internal Server Error",
            Status::NotImplemented => "Not Implemented",
            Status::BadGateway => "Bad Gateway",
            Status::ServiceUnavailable => "Service Unavailable",
            Status::GatewayTimeout => "Gateway Timeout",
            Status::HttpVersionNotSupported => "HTTP Version Not Supported",
            Status::VariantAlsoNegotiates => "Variant Also Negotiates",
            Status::InsufficientStorage => "Insufficient Storage",
            Status::LoopDetected => "Loop Detected",
            Status::NotExtended => "Not Extended",
            Status::NetworkAuthenticationRequired => "Network Authentication Required",
        }
    }
}

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

pub struct Response {
    pub status: Status,
    pub body: Vec<u8>,
    pub content_type: ContentType,
    pub headers: HashMap<String, String>,
}

impl Response {
    pub fn new(status: Status, body: impl Into<Vec<u8>>, content_type: ContentType) -> Self {
        Self {
            status,
            body: body.into(),
            content_type,
            headers: HashMap::new(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {} {}\r\n",
            self.status.as_u16(),
            self.status.reason_phrase()
        );

        // Add Content-Type header
        response.push_str(&format!("Content-Type: {}\r\n", self.content_type.as_str()));

        // Add Content-Length header
        response.push_str(&format!("Content-Length: {}\r\n", self.body.len()));

        // Add custom headers from the collection
        for (key, value) in &self.headers {
            response.push_str(&format!("{}: {}\r\n", key, value));
        }

        response.push_str("\r\n");
        let mut response_bytes = response.into_bytes();
        response_bytes.extend(&self.body);
        response_bytes
    }

    pub fn header(&mut self, key: String, value: String) -> &mut Self {
        self.headers.entry(key).or_insert(value);
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
}
