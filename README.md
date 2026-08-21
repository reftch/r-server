# Reactive Http Server

A modular, high-performance HTTP/1.1 server implementation in Rust featuring an asynchronous engine and Trie-based routing.

## Features

- **Asynchronous Engine**: Uses non-blocking I/O for efficient concurrent connection handling.
- **High-Performance Routing**: Trie-based router with support for dynamic path parameters (e.g., `/users/:id`).
- **Middleware**: Pluggable request pipeline via `fn(&Request, &mut Response, Next)`, chained with `use_middleware` to run logic (logging, timing, auth) for every request before it reaches the handler.
- **Static File Serving**: Built-in support for serving assets from a designated directory with automatic MIME type detection.
- **Modular Architecture**: Separated into specialized modules for requests, responses, routing, and server logic.
- **Client**: A HTTP client capable of making GET, POST, PUT, PATCH, and DELETE requests. It supports both HTTP and HTTPS via SSL/TLS.

## Project Structure

The project is organized into several core crates:

### `client`

Handles client requests to a different endpoint.

### `request`

Handles parsing and representation of incoming HTTP requests from raw byte buffers into structured data including methods, headers, query parameters, and path parameters.

### `response`

Manages the construction of HTTP responses. It provides an expressive Builder-style interface for setting status codes, body content, MIME types, and custom headers compliant with HTTP/1.1.

### `router`

Implements a high-performance Trie-based router that supports both static paths and dynamic parameters (e.g., `/users/:id`). It efficiently matches requests to handlers in $O(path\_length)$ time regardless of the number of routes. It also provides a middleware pipeline: every request passes through registered middleware (via `use_middleware`) before reaching its handler, and through the same pipeline for static file serving.

### `server`

The core engine managing TCP listeners, non-blocking I/O via system polling, and orchestrating the request-response lifecycle.

### `sslserver`

A wrapper around the server logic providing HTTPS support by handling TLS handshakes and certificate management via OpenSSL.

### `logger`

A thread-safe, asynchronous logging system. Messages are processed on a background worker thread to ensure that I/O operations for logging do not block the main request processing loop.

### `utils`

Provides shared utility functions such as environment variable parsing with default fallbacks.

## Usage Example

### HTTP (Standard)

Here is a comprehensive example demonstrating how to initialize the server, configure static assets, add dynamic routes, and construct responses:

```rust
use r_server::server::http::Server;

fn main() -> std::io::Result<()> {
    Server::new()?.run()?;
    Ok(())
}
```

As a result server will started with default host (127.0.0.1) and port (8080) values:

```sh
[2026-08-21 06:54:36.524] [INFO] [r_server::server::http] - Server started on http://127.0.0.1:8080 in 18µs
```

### HTTPS (SSL/TLS)

To use the `sslserver`, you need to provide `key.pem` and `cert.pem` in your working directory. You can generate these using OpenSSL:

```bash
openssl req -x509 -noenc -keyout key.pem -out cert.pem -subj /CN=0.0.0.0
```

The usage pattern is nearly identical, but you use the `server::https::Server` instead of `server::http::Server`:

```rust
use r_server::server::https::Server;

fn main() -> std::io::Result<()> {
    Server::new()?.run()?;
    Ok(())
}
```

As a result server will started with default host (127.0.0.1) and port (8443) values:

```sh
[2026-08-21 06:54:36.524] [INFO] [r_server::server::http] - HTTPS server started on https://127.0.0.1:8443 in 1250µs
```

To connect to the HTTPS server, you can use `curl` with the `-k` flag (to ignore self-signed certificate warnings):

```bash
curl -k https://localhost:8443
```

### `bind(...)`

Both the HTTP (`server::http::Server`) and HTTPS (`server::https::Server`) servers expose an identical `bind(host, port)`
method. It re-binds the underlying `TcpListener` to the supplied `host` and `port` and returns a `&mut Self`, so it can be chained before `run()`.

By default, when no `bind` call is made, the servers use the `HOST`/`PORT` environment variables, falling back to:

- **HTTP** (`server::Server`): `127.0.0.1:8080`
- **HTTPS** (`sslserver::Server`): `127.0.0.1:8443`

Calling `bind` overrides this default address.

```rust
use r_server::server::http::Server;

fn main() -> std::io::Result<()> {
    Server::new()?.bind("0.0.0.0", 8080).run()?;
    Ok(())
}
```

For HTTPS the call is identical — only the import changes — and it overrides the `127.0.0.1:8443` default:

```rust
use r_server::server::https::Server;

fn main() -> std::io::Result<()> {
    Server::new()?.bind("0.0.0.0", 8080).run()?;
    Ok(())
}
```

### Client

A simple HTTP client capable of making GET, POST, PUT, PATCH, and DELETE requests. It supports both HTTP and HTTPS via SSL/TLS.

#### Usage Example

```rust
use r_server::client::Client;

fn main() -> std::io::Result<()> {
    let client = Client::new("https://api.example.com");

    // GET request
    let body = client.get("/api/v1/resource").unwrap();
    println!("Response: {}", body);

    // POST request
    let post_body = r#"{"key": "value"}"#.to_string();
    let post_response = client.post("/api/v1/resource", post_body).unwrap();
    println!("POST Response: {}", post_response);

    Ok(())
}
```

### Middleware

Both the HTTP (`server::Server`) and HTTPS (`sslserver::Server`) servers support a global
middleware pipeline. A middleware is a plain function with the signature
`fn(&Request, &mut Response, Next)`. Each middleware receives the current request, a mutable
response, and a `Next` value. Calling `next.run(req, res)` invokes the remaining middleware
and, finally, the matched route handler. Middleware runs in registration order and applies to
every request, including static file serving.

Both `use_middleware` and `route` return `&mut Self`, so they can be chained after `Server::new()`.

```rust
use r_server::{
    info, logger,
    request::Request,
    response::{self, Response},
    router::{Method, Next},
    server::Server,
};

// A middleware that logs each request with its elapsed handling time.
fn logger(req: &Request, res: &mut Response, next: Next) {
    // pre- actions
    let start = std::time::Instant::now();
    // handling request
    next.run(req, res);
    // post- actions
    info!("{} {} - {:?}", req.method, req.path, start.elapsed());
}

fn main() -> std::io::Result<()> {
    r_server::logger::set_level(logger::LogLevel::Info);

    Server::new()?
        .use_middleware(logger)
        .route(Method::GET, "/api/v1/users/:id", |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                   .body(format!("{{\"value\":{}}}", id));
            }
        })
        .run()?;

    Ok(())
}
```

### Static resources

By default server try to find local directory `assets` and find there index.html. For example:

| Directory                            | Path                                 |
| ------------------------------------ | ------------------------------------ |
| `./assets/index.html`                | http://localhost:8080                |
| `./assets/home/index.html`           | http://localhost:8080/home           |
| `./assets/home/dashboard/index.html` | http://localhost:8080/home/dashboard |

## API Reference Summary

### `Server`

| Method                                              | Description                                                                                                                                                                                                            |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `new() -> IoResult<Self>`                           | Creates a new server instance. Reads `HOST`/`PORT` env vars, defaulting to `0.0.0.0:8080` for the HTTP server and `0.0.0.0:8443` for the HTTPS server.                                                                 |
| `bind(host: &str, port: u16)`                       | Re-binds the underlying TCP listener to a new host/port and returns a mutable reference to the server, overriding the default address chosen by `new()`. Useful for changing the listening address after construction. |
| `assets_path(path: &str)`                           | Sets the directory for serving static files.                                                                                                                                                                           |
| `route(method, path, handler)`                      | Registers a new route with a specific HTTP method and path.                                                                                                                                                            |
| `use_middleware(fn(&Request, &mut Response, Next))` | Registers a global middleware that runs for every request (including static file serving), before the route handler. Returns `&mut Self`.                                                                              |
| `run() -> IoResult<()>`                             | Starts the asynchronous event loop.                                                                                                                                                                                    |

### `Response` (Builder Pattern)

Once you have a mutable reference to the response in a handler, you can use:

- `.status(Status)` : Updates the HTTP status code.
- `.body(impl Into<Vec<u8>>)` : Sets the response body.
- `.content_type(ContentType)` : Sets the MIME type.
- `.header(key, value)` : Adds a custom header.

### `Request`

The `Request` object provided to handlers contains:

- `.method`: The HTTP method (e.g., GET, POST).
- `.path`: The requested URL path.
- `.params`: A map of dynamic path parameters (e.g., `:id`).
- `.headers`: A map of request headers.
- `.query_params`: A map of URL query parameters.

## Getting Started

To build the project, ensure you have Rust and Cargo installed, then run:

```bash
cargo build
```

## How to run example with Docker

Run application

```
docker compose -f examples/docker/compose.yml up -d --build
```

Stop application

```
docker compose -f docker/compose.yml down
```
