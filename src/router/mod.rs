use crate::request::Request;
use crate::response::Response;
use std::collections::HashMap;

pub mod method;
pub use self::method::{InvalidMethod, Method};

pub type HandlerResponse<'a> = Response<'a>;
pub type HandlerFn = for<'a> fn(&Request<'a>, &mut Response<'a>);

pub struct Next<'a> {
    rest: &'a [MiddlewareFn],
    handler: HandlerFn,
}

impl<'a> Next<'a> {
    #[inline]
    pub fn run<'b, 'c>(self, req: &'b Request<'a>, resp: &'c mut Response<'a>) {
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

#[derive(Clone, Copy)]
pub struct MiddlewareFn(pub for<'a, 'b, 'c> fn(&'b Request<'a>, &'c mut Response<'a>, Next<'a>));

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

    pub fn use_middleware(
        &mut self,
        middleware: for<'a, 'b, 'c> fn(&'b Request<'a>, &'c mut Response<'a>, Next<'a>),
    ) {
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

    pub fn handle<'a>(
        &'a self,
        request: &Request<'a>,
        response: &mut Response<'a>,
        handler: HandlerFn,
    ) {
        let next = Next {
            rest: &self.middlewares,
            handler,
        };
        next.run(request, response);
    }
}
