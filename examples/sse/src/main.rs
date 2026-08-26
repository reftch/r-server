use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use r_server::{core::http::Server, request::Request, response::Response, router::Method, task};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn home_handler(_req: &Request, res: &mut Response) {
    res.content_type(r_server::response::ContentType::HTML)
        .body(
            r#"<html>
                <head>
                    <title>SSE Example</title>
                    <script src="https://cdn.jsdelivr.net/npm/htmx.org@4.0.0-beta6/dist/htmx.min.js"></script>
                    <script src="https://cdn.jsdelivr.net/npm/htmx.org@4.0.0-beta6/dist/ext/hx-sse.min.js"></script>
                </head>
                <body style="background-color: #222; color: #eee;">
                <div
                    hx-sse:connect="/stream"
                    hx-target='#count'
                    style="display: flex; gap: 5px;"
                >
                    <span>Count:</span><span id="count"></span>
                </div>
                </body>
            </html>"#,
        );
}

fn main() -> std::io::Result<()> {
    Server::new()?
        .route(Method::GET, "/", home_handler)
        .route(Method::GET, "/stream", |req, res| {
            task::repeat_every(
                req.path.to_string(),
                &*res.metadata,
                Duration::from_millis(500),
                |res| {
                    let _ = res.stream(&format!(
                        "{}\n\n",
                        COUNTER.fetch_add(1, Ordering::Relaxed) + 1
                    ));
                },
            );
        })
        .run()
}
