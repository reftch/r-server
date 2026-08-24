# Reactive Http Server

A modular, high-performance HTTP/1.1 server implementation in Rust featuring an asynchronous engine and Trie-based routing.

## Features

- **Asynchronous Engine**: Uses non-blocking I/O for efficient concurrent connection handling.
- **Multi-Worker Architecture**: Configurable number of worker threads (`workers(n)`), each running an independent event loop with the kernel distributing connections across them.
- **High-Performance Routing**: Trie-based router with support for dynamic path parameters (e.g., `/users/:id`).
- **Middleware**: Pluggable request pipeline via `fn(&Request, &mut Response, Next)`, chained with `use_middleware` to run logic (logging, timing, auth) for every request before it reaches the handler.
- **Static File Serving**: Built-in support for serving assets from a designated directory with automatic MIME type detection.
- **Multipart Support**: Built-in parsing of `multipart/form-data` requests for handling file uploads and form fields.
- **Server-Side Sessions**: Optional browser sessions backed by a thread-safe store shared across workers. Each request receives a cloneable `Session` handle; cookies (`HttpOnly`, `SameSite=Lax`) are managed automatically.
- **Modular Architecture**: Separated into specialized modules for requests, responses, routing, and server logic.
- **Client**: A HTTP client capable of making GET, POST, PUT, PATCH, and DELETE requests. It supports both HTTP and HTTPS via SSL/TLS.

## Project Structure

The project is organized into several core crates:

### `client`

Handles client requests to a different endpoint.

### `request`

Handles parsing and representation of incoming HTTP requests from raw byte buffers into structured data including methods, headers, query parameters, and path parameters. Also includes unified form parsing for both `application/x-www-form-urlencoded` and `multipart/form-data` bodies (`FormField`, `get_form_fields`, `get_form_field`, `get_form_file`).

### `response`

Manages the construction of HTTP responses. It provides an expressive Builder-style interface for setting status codes, body content, MIME types, and custom headers compliant with HTTP/1.1.

### `router`

Implements a high-performance Trie-based router that supports both static paths and dynamic parameters (e.g., `/users/:id`). It efficiently matches requests to handlers in $O(path\_length)$ time regardless of the number of routes. It also provides a middleware pipeline: every request passes through registered middleware (via `use_middleware`) before reaching its handler, and through the same pipeline for static file serving.

### `session`

Server-side browser sessions. A `SessionStore` owns all live sessions and is shared across worker threads. For each request the connection layer resolves the session from the `Cookie` header (`SID` cookie) or mints a fresh one, and attaches a cheap, cloneable `Session` handle to the request. Sessions idle longer than the configured TTL are swept automatically; session ids are generated with OpenSSL's CSPRNG.

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
use r_server::core::http::Server;

fn main() -> std::io::Result<()> {
    Server::new()?.run()?;
    Ok(())
}
```

As a result server will started with default host (127.0.0.1) and port (8080) values:

```sh
[2026-08-21 06:54:36.524] [INFO] [r_server::core::http] - Server started on http://127.0.0.1:8080 in 18µs
```

### HTTPS (SSL/TLS)

To use the `sslserver`, you need to provide `key.pem` and `cert.pem` in your working directory. You can generate these using OpenSSL:

```bash
openssl req -x509 -noenc -keyout key.pem -out cert.pem -subj /CN=0.0.0.0
```

The usage pattern is nearly identical, but you use the `server::https::Server` instead of `server::http::Server`:

```rust
use r_server::core::https::Server;

fn main() -> std::io::Result<()> {
    Server::new()?.run()?;
    Ok(())
}
```

As a result server will started with default host (127.0.0.1) and port (8443) values:

```sh
[2026-08-21 06:54:36.524] [INFO] [r_server::core::http] - HTTPS server started on https://127.0.0.1:8443 in 1250µs
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
use r_server::core::http::Server;

fn main() -> std::io::Result<()> {
    Server::new()?.bind("0.0.0.0", 8080).run()?;
    Ok(())
}
```

For HTTPS the call is identical — only the import changes — and it overrides the `127.0.0.1:8443` default:

```rust
use r_server::core::https::Server;

fn main() -> std::io::Result<()> {
    Server::new()?.bind("0.0.0.0", 8080).run()?;
    Ok(())
}
```

### Workers

Both the HTTP (`server::http::Server`) and HTTPS (`server::https::Server`) servers can run on multiple worker threads via `workers(n)`.
Each worker runs an independent event loop with its own connection set, and the OS distributes incoming connections across
workers (via `SO_REUSEPORT` on Linux, a shared listening socket elsewhere). The method returns `&mut Self`, so it can be chained
before `run()`. Defaults to a single worker.

```rust
use r_server::core::http::Server;

fn main() -> std::io::Result<()> {
    Server::new()?.workers(4).run()?;
    Ok(())
}
```

The startup log reports how many workers are running:

```sh
[2026-08-21 06:54:36.524] [INFO] [r_server::core::http] - HTTP server started on http://127.0.0.1:8080 with 4 worker(s) in 18µs
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
    core::Server,
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

### Form Parameters

The server parses `application/x-www-form-urlencoded` bodies out of the box. In a handler, call
`req.get_form_field("name")` to read a single text field, or `req.get_form_fields()` to get all of
them as a `Vec<FormField>`. Both accessors also work on `multipart/form-data` bodies (see the next
section), so the same handler code can serve either encoding.

A runnable version of this example lives in `examples/forms/form`.

```rust
use r_server::{core::http::Server, response::Status, router::Method};

fn main() -> std::io::Result<()> {
    Server::new()?
        .route(Method::GET, "/", |_req, res| {
            res.content_type(r_server::response::ContentType::HTML).body(
                r#"<html>
                    <body>
                        <form action="/post" method="POST">
                            <label for="name">Name:</label>
                            <input name="name">
                            <button type="submit">Submit form</button>
                        </form>
                    </body>
                </html>"#,
            );
        })
        .route(Method::POST, "/post", |req, res| {
            match req.get_form_field("name") {
                Ok(name) => res.body(format!("Your name is {name}")),
                Err(e) => res.status(Status::BadRequest).body(e),
            }
        })
        .run()?;

    Ok(())
}
```

You can also test it with `curl`:

```bash
curl -d "name=Alice" http://localhost:8080/post
```

### Multipart (File Uploads)

The server parses `multipart/form-data` request bodies out of the box. Every parsed field is a
`FormField` containing:

- `name`: The form field name (`name` attribute from `Content-Disposition`).
- `filename`: The original filename for uploaded files (`None` for plain form fields).
- `content_type`: The MIME type of the part, if provided.
- `data`: The raw bytes of the part.

Use `req.get_form_file("file")` to fetch a file upload by field name. It returns
`Err` when the field is missing, is a text field, or when no file was selected (browsers submit
file inputs without a selection as parts with `filename=""`), which makes it easy to respond with
a 400.

A runnable version of this example lives in `examples/forms/multipart`.

```rust
use r_server::{core::http::Server, info, response::{ContentType, Status}, router::Method};

fn main() -> std::io::Result<()> {
    Server::new()?
        .route(Method::GET, "/", |_req, res| {
            // Simple upload form
            res.content_type(ContentType::HTML).body(
                r#"<html>
                    <head><title>Upload Test</title></head>
                    <body>
                        <form target="/" method="post" enctype="multipart/form-data">
                            <input type="file" multiple name="file"/>
                            <button type="submit">Submit</button>
                        </form>
                    </body>
                </html>"#,
            );
        })
        .route(Method::POST, "/", |req, res| {
            match req.get_form_file("file") {
                Ok(file) => {
                    let filename = file.filename.unwrap_or_else(|| "upload.bin".into());
                    info!("saving {filename} ({} bytes)", file.data.len());
                    std::fs::write(&filename, &file.data).expect("failed to write file");
                    res.body("Uploaded");
                }
                Err(e) => res.status(Status::BadRequest).body(e),
            }
        })
        .run()?;

    Ok(())
}
```

You can also test it with `curl`:

```bash
curl -F "file=@photo.jpg" http://localhost:8080/
```

### Sessions

The server supports optional server-side sessions. Enable them with `sessions_ttl(secs)` — every
parsed request then receives a session handle at `req.session()`. The session is resolved from the
`SID` cookie (or minted fresh for new visitors), and the server sets/expires the cookie
automatically after dispatch (`HttpOnly`, `SameSite=Lax`, `Max-Age` equal to the TTL). Sessions are
shared state: all methods take `&self`, so handlers mutate them through interior mutability.

- `session.id()`: The opaque session identifier stored in the browser cookie.
- `session.get(key) -> Option<String>` / `session.set(key, value)` / `session.remove(key)`: Arbitrary per-session data.
- `session.destroy()`: Marks the session as ended; the store evicts it and the browser cookie is expired.

A runnable version of this example lives in `examples/session`.

```rust
use r_server::{
    core::http::Server,
    request::Request,
    response::{ContentType, Response, Status},
    router::Method,
};

fn login(req: &Request, res: &mut Response) {
    match (req.get_form_field("username"), req.session()) {
        (Ok(username), Some(session)) => {
            session.set("user_id", username);
            res.status(Status::MovedTemporarily)
                .header("Location", "/")
                .body("Redirecting...");
        }
        (_, None) => res.status(Status::InternalServerError).body("sessions are not enabled"),
        (Err(_), _) => res.status(Status::BadRequest).body("missing 'username' field"),
    }
}

fn main() -> std::io::Result<()> {
    Server::new()?
        .sessions_ttl(3600) // sessions survive 1h of inactivity
        .route(Method::GET, "/", |req, res| {
            let user = req.session().and_then(|s| s.get("user_id"));
            let page = match user {
                Some(user) => format!("<h1>Hello, {user}</h1>"),
                None => "<h1>Guest</h1>".to_string(),
            };
            res.content_type(ContentType::HTML).body(page);
        })
        .route(Method::POST, "/login", login)
        .route(Method::POST, "/logout", |req, res| {
            if let Some(session) = req.session() {
                session.destroy();
            }
            res.status(Status::MovedTemporarily)
                .header("Location", "/")
                .body("Redirecting...");
        })
        .run()?;

    Ok(())
}
```

You can also test it with `curl`:

```bash
# Log in; -c stores the session cookie
curl -d "username=Alice" -c cookies.txt http://localhost:8080/login

# Subsequent requests carry the cookie and resolve the same session (-b sends it)
curl -b cookies.txt http://localhost:8080/
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

| Method                                              | Description                                                                                                                                                                                                                                                    |
| --------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `new() -> IoResult<Self>`                           | Creates a new server instance. Reads `HOST`/`PORT` env vars, defaulting to `0.0.0.0:8080` for the HTTP server and `0.0.0.0:8443` for the HTTPS server.                                                                                                         |
| `bind(host: &str, port: u16)`                       | Re-binds the underlying TCP listener to a new host/port and returns a mutable reference to the server, overriding the default address chosen by `new()`. Useful for changing the listening address after construction.                                         |
| `assets_path(path: &str)`                           | Sets the directory for serving static files.                                                                                                                                                                                                                   |
| `workers(n: usize)`                                 | Sets the number of worker threads. Each worker runs an independent event loop; the OS distributes connections across them (`SO_REUSEPORT` on Linux, a shared listening socket elsewhere). Values below 1 are clamped to 1. Defaults to 1. Returns `&mut Self`. |
| `route(method, path, handler)`                      | Registers a new route with a specific HTTP method and path.                                                                                                                                                                                                    |
| `use_middleware(fn(&Request, &mut Response, Next))` | Registers a global middleware that runs for every request (including static file serving), before the route handler. Returns `&mut Self`.                                                                                                                                                                                      |
| `sessions_ttl(ttl_secs: u64)`                       | Enables server-side browser sessions with the given idle timeout. Every parsed request receives a session handle at `request.session()`; sessions are resolved from the `Cookie` header or minted fresh, and a new session is announced to the browser via a `Set-Cookie` header on the response. Disabled by default. Returns `&mut Self`. |
| `run() -> IoResult<()>`                             | Starts the asynchronous event loop.                                                                                                                                                                                                                                                                                            |

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
- `.get_form_fields() -> Result<Vec<FormField>, String>`: Parses the submitted form, dispatching on `Content-Type` (`application/x-www-form-urlencoded` or `multipart/form-data`).
- `.get_form_field(name) -> Result<String, String>`: Gets a text field by name from either encoding. Errors if the field is missing or is a file upload.
- `.get_form_file(name) -> Result<FormField, String>`: Gets a file-upload field by name (multipart forms only). Errors if the field is missing, is a text field, or if no file was selected (`filename=""`).
- `.session: Option<Session>` / `.session() -> Option<&Session>`: The browser session handle attached by the server when sessions were enabled with `Server::sessions_ttl`; `None` otherwise. See the Sessions section.

Each `FormField` contains:

- `.name`: The form field name.
- `.filename`: The original filename for uploaded files (`None` for text fields).
- `.content_type`: The MIME type of the part, if provided.
- `.data`: The raw bytes of the part.
- `.text()`: The payload decoded as UTF-8 (invalid bytes are replaced).

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
