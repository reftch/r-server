use crate::request::Request;
use crate::response::Response;
use std::collections::HashMap;

pub mod method;
pub use self::method::{InvalidMethod, Method};

pub type HandlerResponse<'a, T> = Response<'a, T>;
pub type HandlerFn<T> = fn(&Request, &mut Response<T>);

const METHOD_COUNT: usize = 7;

struct ParamChild<T> {
    name: Box<str>,
    node: Box<TrieNode<T>>,
}

struct TrieNode<T> {
    children: HashMap<Box<str>, Box<TrieNode<T>>>,
    param_child: Option<ParamChild<T>>,
    handlers: [Option<HandlerFn<T>>; METHOD_COUNT],
}

impl<T> TrieNode<T> {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            param_child: None,
            handlers: [None; METHOD_COUNT],
        }
    }
}

pub struct Router<T> {
    root: TrieNode<T>,
}

impl<T> Default for Router<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Router<T> {
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    pub fn add_route(&mut self, method: Method, path: &str, handler: HandlerFn<T>) {
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
    pub fn route<'a>(&'a self, request: &mut Request<'a>) -> Option<HandlerFn<T>> {
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
