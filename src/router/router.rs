use crate::request::Request;
use crate::response::{ContentType, Response, Status};
use crate::trace;
use std::collections::HashMap;

pub type HandlerResponse = Response;
pub type HandlerFn = fn(&Request, &mut Response);
use std::fmt;

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

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

const METHOD_COUNT: usize = 7;

struct ParamChild {
    name: Box<str>,
    node: Box<TrieNode>,
}

struct TrieNode {
    children: HashMap<Box<str>, Box<TrieNode>>,
    param_child: Option<ParamChild>,
    handlers: [Option<HandlerFn>; METHOD_COUNT],
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            param_child: None,
            handlers: [None; METHOD_COUNT],
        }
    }
}

pub struct Router {
    root: TrieNode,
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    pub fn add_route(&mut self, method: Method, path: &str, handler: HandlerFn) {
        let mut current = &mut self.root;

        for part in path.split('/').filter(|s| !s.is_empty()) {
            if let Some(name) = part.strip_prefix(':') {
                let pc = current.param_child.get_or_insert_with(|| ParamChild {
                    name: name.into(),
                    node: Box::new(TrieNode::new()),
                });

                current = pc.node.as_mut();
            } else {
                current = current
                    .children
                    .entry(part.into())
                    .or_insert_with(|| Box::new(TrieNode::new()))
                    .as_mut();
            }
        }

        current.handlers[method.index()] = Some(handler);
    }

    pub fn route<'a>(&'a self, request: &mut Request<'a>) -> Option<Response> {
        let mut current = &self.root;

        for part in request.path.split('/').filter(|s| !s.is_empty()) {
            if let Some(next) = current.children.get(part) {
                current = next.as_ref();
            } else if let Some(pc) = &current.param_child {
                trace!("Extracting param: {} = '{}'", pc.name, part);
                request.params.push((pc.name.as_ref(), part));
                // request.params.insert(pc.name.as_ref(), part);
                current = pc.node.as_ref();
            } else {
                return None;
            }
        }

        let method: Method = request.method.parse().expect("Failed to parse");

        trace!("Looking up handler for method: {}", method.to_string());
        let handler = current.handlers[method.index()]?;

        trace!("Handler found. Executing...");
        let mut response = Response::new(Status::Ok, b"", ContentType::TEXT);
        handler(request, &mut response);

        Some(response)
    }
}
