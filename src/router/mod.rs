pub mod router;

pub use self::router::{HandlerFn, InvalidMethod, Method, Router};

#[cfg(test)]
mod tests;
