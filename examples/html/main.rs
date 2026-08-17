use r_server::{logger, response, router::Method, server::http::Server, task};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn main() -> std::io::Result<()> {
    r_server::logger::set_level(logger::LogLevel::Info);

    Server::new()?
        .route(Method::GET, "/api/v1/users/:id", move |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .route(Method::GET, "/stream", move |req, res| {
            task::repeat_every(
                req.path,
                res.metadata.try_clone().unwrap(),
                Duration::from_millis(50),
                move |res| {
                    let _ = res.stream(&format!(
                        "{}\n\n",
                        COUNTER.fetch_add(1, Ordering::Relaxed) + 1
                    ));
                },
            );
        })
        .assets_path("./examples/html/assets")
        .run()?;

    Ok(())
}
