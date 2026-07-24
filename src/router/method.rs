#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Method {
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
    HEAD,
    OPTIONS,
}

// A simple error type for when parsing fails
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidMethod;

impl Method {
    /// Returns the integer index of the method.
    /// This is now extremely fast because it's a direct cast.
    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Method::GET => write!(f, "GET"),
            Method::POST => write!(f, "POST"),
            Method::PUT => write!(f, "PUT"),
            Method::PATCH => write!(f, "PATCH"),
            Method::DELETE => write!(f, "DELETE"),
            Method::HEAD => write!(f, "HEAD"),
            Method::OPTIONS => write!(f, "OPTIONS"),
        }
    }
}

// This is what allows you to use .parse() and the '?' operator correctly.
impl std::str::FromStr for Method {
    type Err = InvalidMethod; // We define that this parser returns an InvalidMethod error

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" => Ok(Method::GET),
            "POST" => Ok(Method::POST),
            "PUT" => Ok(Method::PUT),
            "PATCH" => Ok(Method::PATCH),
            "DELETE" => Ok(Method::DELETE),
            "HEAD" => Ok(Method::HEAD),
            "OPTIONS" => Ok(Method::OPTIONS),
            _ => Err(InvalidMethod), // Return an error instead of None
        }
    }
}
