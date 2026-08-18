use crate::request::Request;
use crate::response::Response;
use std::collections::HashMap;

pub mod method;
pub use self::method::{InvalidMethod, Method};

pub type HandlerResponse<'a> = Response<'a>;
pub type HandlerFn = for<'a> fn(&Request<'a>, &mut Response<'a>);

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
}

#[cfg(test)]
mod tests;
