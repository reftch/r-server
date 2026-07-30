use crate::router::{HandlerFn, Method};
use std::io;
use std::net::TcpStream;

pub mod connection;
pub mod http;
pub mod https;

/// Common interface implemented by all server types.
pub trait ServerCore {
    type Stream;

    fn route(&mut self, method: Method, path: &str, handler: HandlerFn<Self::Stream>);

    fn assets_path(&mut self, path: &str);

    fn run(&mut self) -> io::Result<()>;
}

/// Generic server builder.
pub struct ServerBuilder<S>
where
    S: ServerCore,
{
    server: S,
}

impl<S> ServerBuilder<S>
where
    S: ServerCore,
{
    /// Creates a builder from an existing server.
    pub fn new(server: S) -> Self {
        Self { server }
    }

    /// Register a route.
    pub fn route(mut self, method: Method, path: &str, handler: HandlerFn<S::Stream>) -> Self {
        self.server.route(method, path, handler);
        self
    }

    /// Configure the static assets directory.
    pub fn assets_path(mut self, path: &str) -> Self {
        self.server.assets_path(path);
        self
    }

    /// Start the server.
    pub fn run(mut self) -> io::Result<()> {
        self.server.run()
    }
}

/// Convenience aliases.
pub type HttpServer = ServerBuilder<http::Server>;
pub type HttpsServer = ServerBuilder<https::Server>;

impl ServerCore for http::Server {
    type Stream = TcpStream;

    fn route(&mut self, method: Method, path: &str, handler: HandlerFn<Self::Stream>) {
        http::Server::route(self, method, path, handler);
    }

    fn assets_path(&mut self, path: &str) {
        http::Server::assets_path(self, path);
    }

    fn run(&mut self) -> io::Result<()> {
        http::Server::run(self)
    }
}

impl ServerCore for https::Server {
    type Stream = Option<https::TlsState>;

    fn route(&mut self, method: Method, path: &str, handler: HandlerFn<Self::Stream>) {
        https::Server::route(self, method, path, handler);
    }

    fn assets_path(&mut self, path: &str) {
        https::Server::assets_path(self, path);
    }

    fn run(&mut self) -> io::Result<()> {
        https::Server::run(self)
    }
}
