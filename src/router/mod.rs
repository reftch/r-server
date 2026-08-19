use crate::request::Request;
use crate::response::Response;
use std::collections::HashMap;

pub mod method;
pub use self::method::{InvalidMethod, Method};

pub type HandlerResponse<'a> = Response<'a>;
pub type HandlerFn = for<'a> fn(&Request<'a>, &mut Response<'a>);

// Wrap the function pointer in a struct to break the recursive type alias cycle.
#[derive(Clone, Copy)]
pub struct MiddlewareFn(
    pub for<'a> fn(&Request<'a>, &mut Response<'a>, &[MiddlewareFn], HandlerFn),
);

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
    middlewares: Vec<MiddlewareFn>,
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
            middlewares: Vec::new(),
        }
    }

    pub fn use_middleware(&mut self, middleware: MiddlewareFn) {
        self.middlewares.push(middleware);
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

    #[inline]
    pub fn route<'a>(&'a self, request: &mut Request<'a>) -> Option<HandlerFn> {
        let mut current = &self.root;

        let path = request
            .path
            .split_once('?')
            .map_or(request.path, |(p, _)| p);

        for part in path.split('/') {
            if part.is_empty() {
                continue;
            }

            match current.children.get(part) {
                Some(next) => {
                    current = next;
                }
                None => {
                    let param = current.param_child.as_ref()?;

                    request.params.push((param.name.as_ref(), part));

                    current = &param.node;
                }
            }
        }

        let method: Method = request.method.parse().expect("Failed to parse");

        current.handlers[method.index()]
    }

    /// Dispatcher helper to advance the middleware slice stack
    pub fn next<'a>(
        req: &Request<'a>,
        resp: &mut Response<'a>,
        rest: &[MiddlewareFn],
        handler: HandlerFn,
    ) {
        if let Some((current_mw, next_mws)) = rest.split_first() {
            // Call the inner function pointer inside the struct wrapper
            (current_mw.0)(req, resp, next_mws, handler);
        } else {
            handler(req, resp);
        }
    }

    /// Entry point to process request through middlewares
    pub fn handle<'a>(
        &'a self,
        request: &Request<'a>,
        response: &mut Response<'a>,
        handler: HandlerFn,
    ) {
        Self::next(request, response, &self.middlewares, handler);
    }
}
