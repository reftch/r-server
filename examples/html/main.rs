use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use r_server::{logger, response, router::Method, server::http::Server, task};

fn main() -> std::io::Result<()> {
    r_server::logger::set_level(logger::LogLevel::Info);
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    Server::new("0.0.0.0:8082")?
        .route(Method::GET, "/api/v1/users/:id", move |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .route(Method::GET, "/stream", move |req, res| {
            task::repeat_every(
                req.path,
                res.metadata.stream.try_clone().unwrap(),
                Duration::from_secs(1),
                move |response| {
                    let _ = response.stream(&format!(
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
