use std::{thread, time::Duration};

use r_server::{
    response::{self, ContentType, Response, Status},
    router::Method,
    server::{connection::ConnectionMetadata, http::Server},
};

fn main() -> std::io::Result<()> {
    // r_server::logger::set_level(logger::LogLevel::Trace);
    Server::new("127.0.0.1:8082")?
        .route(Method::GET, "/api/v1/users/:id", |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .route(Method::GET, "/stream", |_, res| {
            res.content_type(response::ContentType::SSE);

            let stream = res.metadata.stream.try_clone().expect("Error cloning");
            thread::spawn(move || {
                let conn = ConnectionMetadata { stream };
                let response = Response::new(&conn, Status::Ok, b"", ContentType::SSE);

                let mut i = 0;
                loop {
                    i += 1;
                    let message = format!("data: {}\n\n", i);
                    println!("{}", message);
                    response.sse(&message);
                    thread::sleep(Duration::from_secs(1));
                }
            });
        })
        .assets_path("./examples/html/assets")
        .run()?;

    Ok(())
}
