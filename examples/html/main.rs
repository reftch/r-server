use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

use r_server::{
    logger,
    response::{self, ContentType, Response, Status},
    router::Method,
    server::{connection::ConnectionMetadata, http::Server},
    task,
};

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
            let conn = ConnectionMetadata {
                stream: res
                    .metadata
                    .stream
                    .try_clone()
                    .expect("Error cloning stream"),
            };

            thread::spawn(move || {
                let response = Response::new(&conn, Status::Ok, b"", ContentType::SSE);
                let _ = response.stream(&format!(
                    "{}\n\n",
                    COUNTER.fetch_add(1, Ordering::Relaxed) + 1
                ));
                thread::sleep(Duration::from_millis(500));
            });

            // task::repeat_every(
            //     req.path,
            //     res.metadata.stream.try_clone().unwrap(),
            //     Duration::from_millis(500),
            //     move |res| {
            //         let _ = response.stream(&format!(
            //             "{}\n\n",
            //             COUNTER.fetch_add(1, Ordering::Relaxed) + 1
            //         ));
            //     },
            // );
        })
        .assets_path("./examples/html/assets")
        .run()?;

    Ok(())
}
