use crate::request::Request;
use crate::response::Response;
use std::{collections::HashMap, path::PathBuf};

pub mod method;
pub use self::method::{InvalidMethod, Method};

/// Type alias for a Response used within a handler.
pub type HandlerResponse = Response;

/// A function signature for request handlers.
pub type HandlerFn = fn(&Request, &mut Response);

/// A function signature for static request handlers.
pub type HandlerStaticFn = fn(&Request, &mut Response, &PathBuf);

/// Represents the chain of execution for middleware.
pub struct Next<'a> {
    /// The remaining middleware functions in the chain.
    rest: &'a [MiddlewareFn],
    /// The final request handler or closure to execute.
    handler: &'a dyn Fn(&Request, &mut Response),
}

impl<'a> Next<'a> {
    /// Executes the middleware chain.
    #[inline]
    pub fn run(self, req: &Request, resp: &mut Response) {
        if let Some((current_mw, next_mws)) = self.rest.split_first() {
            let next = Next {
                rest: next_mws,
                handler: self.handler,
            };
            (current_mw.0)(req, resp, next);
        } else {
            (self.handler)(req, resp);
        }
    }
}

/// A wrapper around a middleware function signature.
#[derive(Clone, Copy)]
pub struct MiddlewareFn(pub fn(&Request, &mut Response, Next));

/// Constant representing the number of HTTP methods supported by the router.
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

    /// Registers a middleware function that runs for every request.
    pub fn use_middleware(&mut self, middleware: fn(&Request, &mut Response, Next)) {
        self.middlewares.push(MiddlewareFn(middleware));
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
    pub fn route(&self, request: &mut Request) -> Option<HandlerFn> {
        let mut current = &self.root;

        let path = request
            .path
            .split_once('?')
            .map_or(&*request.path, |(p, _)| p);

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
                    request.params.push((param.name.clone(), part.into()));
                    current = &param.node;
                }
            }
        }

        let method: Method = request.method.parse().expect("Failed to parse method");
        current.handlers[method.index()]
    }

    /// Entry point to process standard request handlers.
    pub fn handle(&self, request: &Request, response: &mut Response, handler: HandlerFn) {
        let next = Next {
            rest: &self.middlewares,
            handler: &handler,
        };
        next.run(request, response);
    }

    /// Entry point to process static file handlers through the exact same middleware pipeline.
    pub fn static_handle(
        &self,
        request: &Request,
        response: &mut Response,
        handler: HandlerStaticFn,
        path: &PathBuf,
    ) {
        let closure = |req: &Request, resp: &mut Response| {
            handler(req, resp, path);
        };

        let next = Next {
            rest: &self.middlewares,
            handler: &closure,
        };
        next.run(request, response);
    }
}
