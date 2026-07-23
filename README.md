# Reactive Http Server

A modular, high-performance HTTP/1.1 server implementation in Rust featuring an asynchronous engine and Trie-based routing.

## Features

- **Asynchronous Engine**: Uses non-blocking I/O for efficient concurrent connection handling.
- **High-Performance Routing**: Trie-based router with support for dynamic path parameters (e.g., `/users/:id`).
- **Static File Serving**: Built-in support for serving assets from a designated directory with automatic MIME type detection.
- **Modular Architecture**: Separated into specialized modules for requests, responses, routing, and server logic.

## Project Structure

The project is organized into several core crates:

### `request`
Handles parsing and representation of incoming HTTP requests from raw byte buffers into structured data including methods, headers, query parameters, and path parameters.

### `response`
Manages the construction of HTTP responses. It provides an expressive Builder-style interface for setting status codes, body content, MIME types, and custom headers compliant with HTTP/1.1.

### `router`
Implements a high-performance Trie-based router that supports both static paths and dynamic parameters (e.g., `/users/:id`). It efficiently matches requests to handlers in $O(path\_length)$ time regardless of the number of routes.

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
use r_server::{response, router::Method, server::Server};

fn main() -> std::io::Result<()> {
    Server::new("0.0.0.0:8080")?
        .route(Method::GET, "/api/v1/inc/:id", |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .run()?;

    Ok(())
}
```

### HTTPS (SSL/TLS)

To use the `sslserver`, you need to provide `key.pem` and `cert.pem` in your working directory. You can generate these using OpenSSL:

```bash
openssl req -x509 -noenc -keyout key.pem -out cert.pem -subj /CN=0.0.0.0
```

The usage pattern is nearly identical, but you use the `sslserver::Server` instead of `server::Server`:

```rust
use r_server::{response, router::Method, sslserver::Server};

fn main() -> std::io::Result<()> {
    Server::new("0.0.0.0:8443")?
        .route(Method::GET, "/api/v1/inc/:id", |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .run()?;

    Ok(())
}
```

To connect to the HTTPS server, you can use `curl` with the `-k` flag (to ignore self-signed certificate warnings):

```bash
curl -k https://localhost:8443/api/v1/inc/100
```


## API Reference Summary

### `Server`

| Method | Description |
|--------|-------------|
| `new(addr: &str) -> IoResult<Self>` | Creates a new server instance listening on the given address. |
| `assets_path(path: &str)` | Sets the directory for serving static files. |
| `route(method, path, handler)` | Registers a new route with a specific HTTP method and path. |
| `run() -> IoResult<()>` | Starts the asynchronous event loop. |

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
