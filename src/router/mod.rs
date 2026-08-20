use crate::request::Request;
use crate::response::Response;
use std::collections::HashMap;

pub mod method;
pub use self::method::{InvalidMethod, Method};

/// Type alias for a Response used within a handler.
pub type HandlerResponse = Response;

/// A function signature for request handlers.
/// Takes a read-only reference to a `Request` and a mutable reference to a `Response`.
pub type HandlerFn = fn(&Request, &mut Response);

/// Represents the chain of execution for middleware.
///
/// `Next` allows a middleware function to decide whether to continue the execution
/// chain by calling `next.run()`, or to halt and return early.
pub struct Next<'a> {
    /// The remaining middleware functions in the chain.
    rest: &'a [MiddlewareFn],
    /// The final request handler to be called if all middleware pass control.
    handler: HandlerFn,
}

impl<'a> Next<'a> {
    /// Executes the middleware chain.
    ///
    /// If there are more middlewares in `rest`, the first one is popped and called with a
    /// new `Next` instance containing the remaining chain.
    /// If no middlewares remain, the final `handler` is executed.
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

/// A wrapper around a function that implements the middleware pattern.
///
/// Signature: `fn(&Request, &mut Response, Next)`
#[derive(Clone, Copy)]
pub struct MiddlewareFn(pub fn(&Request, &mut Response, Next));

/// Constant representing the number of HTTP methods supported by the router.
const METHOD_COUNT: usize = 7;

/// A node in the Trie representing a path parameter (e.g., `:id`).
struct ParamChild {
    /// The name of the parameter (e.g., "id").
    name: Box<str>,
    /// The Trie node that this parameter points to.
    node: Box<TrieNode>,
}

/// A node in the Trie representing a segment of a URL path.
struct TrieNode {
    /// Static path segments (e.g., "users", "api").
    children: HashMap<Box<str>, Box<TrieNode>>,
    /// A dynamic path parameter segment (e.g., ":id").
    param_child: Option<ParamChild>,
    /// Handlers mapped to specific HTTP methods for this specific node.
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

/// A high-performance router that uses a Trie (prefix tree) for path matching.
///
/// It supports:
/// - Static path segments.
/// - Path parameters (e.g., `:user_id`).
/// - Middleware execution chain.
/// - HTTP method-based routing.
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
    /// Creates a new, empty `Router`.
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
            middlewares: Vec::new(),
        }
    }

    /// Registers a middleware function that will run for every request handled by this router.
    pub fn use_middleware(&mut self, middleware: fn(&Request, &mut Response, Next)) {
        self.middlewares.push(MiddlewareFn(middleware));
    }

    /// Adds a route to the Trie.
    ///
    /// `path` segments starting with `:` are treated as dynamic parameters.
    /// Example: `add_route(Method::Get, "/user/:id", my_handler)`
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

    /// Matches a request against the Trie and populates request parameters.
    ///
    /// This method strips query parameters before matching and populates
    /// `request.params` with any dynamic segments found during traversal.
    ///
    /// Returns `Some(HandlerFn)` if a match is found, otherwise `None`.
    #[inline]
    pub fn route(&self, request: &mut Request) -> Option<HandlerFn> {
        let mut current = &self.root;

        // Ignore query string for routing purposes
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
                    // If no static match, check if there is a dynamic parameter child
                    let param = current.param_child.as_ref()?;

                    // Store the captured parameter in the request as Box<str>
                    request.params.push((param.name.clone(), part.into()));

                    current = &param.node;
                }
            }
        }

        let method: Method = request.method.parse().expect("Failed to parse method");

        current.handlers[method.index()]
    }

    /// Entry point to process a request through the entire pipeline.
    ///
    /// This triggers the middleware chain, which eventually calls the provided `handler`.
    pub fn handle(&self, request: &Request, response: &mut Response, handler: HandlerFn) {
        let next = Next {
            rest: &self.middlewares,
            handler,
        };
        next.run(request, response);
    }
}
