# Reactive Http Server

A modular, high-performance HTTP/1.1 server implementation in Rust featuring an asynchronous engine and Trie-based routing.

## Features

- **Asynchronous Engine**: Uses non-blocking I/O for efficient concurrent connection handling.
- **High-Performance Routing**: Trie-based router with support for dynamic path parameters (e.g., `/users/:id`).
- **Static File Serving**: Built-in support for serving assets from a designated directory with automatic MIME type detection.
- **Modular Architecture**: Separated into specialized crates for requests, responses, routing, and server logic.

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
use response::{Status, ContentType};
use router::Method;
use server::Server;
use utils::get_env;

fn main() -> std::io::Result<()> {
    // 1. Setup configuration from environment variables
    let addr = format!("{}:{}", get_env("HOST", "0.0.0.0".to_string()), get_env("PORT", 8080));

    // 2. Initialize the server
    let mut server = Server::new(&addr)?;

    // 3. Configure the static assets directory (fallback if no route matches)
    server.set_assets_path("./assets");

    // 4. Add a dynamic route with path parameters
    server.add_route(Method::GET, "/api/v1/inc/:id", |req, res| {
        if let Some(id) = req.params.get("id") {
            if let Ok(val) = id.parse::<i32>() {
                // 5. Construct a successful response
                res.set_status(Status::Ok)
                    .set_content_type(ContentType::JSON)
                    .set_body(format!("{{\"value\":{}}}", val + 1));
            } else {
                res.set_status(Status::BadRequest)
                    .set_body("Invalid ID - must be an integer".to_string());
            }
        }
    });

    // 6. Start the server loop
    server.run()?;

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
use response::{Status, ContentType};
use router::Method;
use sslserver::Server;
use utils::get_env;

fn main() -> std::io::Result<()> {
    // Use a different default port for HTTPS (e.g., 8443)
    let addr = format!("{}:{}", get_env("HOST", "0.0.0.0".to_string()), get_env("PORT", 8443));

    let mut server = Server::new(&addr)?;
    server.set_assets_path("./assets");

    server.add_route(Method::GET, "/api/v1/inc/:id", |req, res| {
        if let Some(id) = req.params.get("id") {
            if let Ok(val) = id.parse::<i32>() {
                res.set_status(Status::Ok)
                    .set_content_type(ContentType::JSON)
                    .set_body(format!("{{\"value\":{}}}", val + 1));
            } else {
                res.set_status(Status::BadRequest)
                    .set_body("Invalid ID - must be an integer".to_string());
            }
        }
    });

    server.run()?;

    Ok(())
}
```

To connect to the HTTPS server, you can use `curl` with the `-k` flag (to ignore self-signed certificate warnings):

```bash
curl -k https://localhost:8443/api/v1/inc/42
```


## API Reference Summary

### `Server`

| Method | Description |
|--------|-------------|
| `new(addr: &str) -> IoResult<Self>` | Creates a new server instance listening on the given address. |
| `set_assets_path(path: &str)` | Sets the directory for serving static files. |
| `add_route(method, path, handler)` | Registers a new route with a specific HTTP method and path. |
| `run() -> IoResult<()>` | Starts the asynchronous event loop. |

### `Response` (Builder Pattern)

Once you have a mutable reference to the response in a handler, you can use:

- `.set_status(Status)` : Updates the HTTP status code.
- `.set_body(impl Into<Vec<u8>>)` : Sets the response body.
- `.set_content_type(ContentType)` : Sets the MIME type.
- `.set_header(key, value)` : Adds a custom header.

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

## Developer Guide

This project uses [mise](https://mise.jdx.dev/) for task orchestration. You can use `mise run <task-name>` to execute the following commands:

### Application Tasks

| Task | Description | Command |
|------|-------------|---------|
| `app` | Build and run application | `cargo run` |
| `dev` | Run in development mode with hot reloading | `watch app` (triggers `mise w`) |
| `test` | Run all tests in the workspace | `cargo test --workspace` |
| `clean` | Clean build artifacts | `cargo clean` |

### Production & Docker Tasks

| Task | Description | Command |
|------|-------------|---------|
| `prod-up` | Bring up production environment (Docker) | `docker compose up -d --build` |
| `prod-down`| Take down production environment | `docker compose down` |

## Getting Started

To build the project, ensure you have Rust and Cargo installed, then run:

```bash
cargo build
```
