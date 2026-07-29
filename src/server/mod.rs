use crate::router::{HandlerFn, Method};
use std::io;

pub mod connection;
pub mod http;
pub mod https;

/// A builder for the standard HTTP server using TcpStream.
pub struct HttpServer {
    server: crate::server::http::Server,
}

impl HttpServer {
    /// Creates a new HttpServerBuilder listening on the given address.
    pub fn new(addr: &str) -> io::Result<Self> {
        Ok(Self {
            server: crate::server::http::Server::new(addr)?,
        })
    }

    /// Sets the directory for serving static files.
    pub fn assets_path(mut self, path: &str) -> Self {
        self.server.assets_path(path);
        self
    }

    /// Adds a route to the server with a standard TcpStream handler.
    pub fn route(
        mut self,
        method: Method,
        path: &str,
        handler: HandlerFn<std::net::TcpStream>,
    ) -> Self {
        self.server.route(method, path, handler);
        self
    }

    /// Starts the server's event loop.
    pub fn run(mut self) -> io::Result<()> {
        self.server.run()
    }
}

/// A builder for the secure HTTPS server.
pub struct HttpsServer {
    server: crate::server::https::Server,
}

impl HttpsServer {
    /// Creates a new HttpsServerBuilder from an existing HTTPS server instance.
    pub fn new(server: crate::server::https::Server) -> Self {
        Self { server }
    }

    /// Sets the directory for serving static files.
    pub fn assets_path(mut self, path: &str) -> Self {
        self.server.assets_path(path);
        self
    }

    /// Adds a route to the server with an Option<TlsState> handler.
    pub fn route(
        mut self,
        method: Method,
        path: &str,
        handler: crate::router::HandlerFn<Option<crate::server::https::TlsState>>,
    ) -> Self {
        self.server.route(method, path, handler);
        self
    }

    /// Starts the server's event loop.
    pub fn run(mut self) -> io::Result<()> {
        self.server.run()
    }
}
