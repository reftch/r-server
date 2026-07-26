use r_server::{logger, response, router::Method, server::Server};

fn main() -> std::io::Result<()> {
    // r_server::logger::set_level(logger::LogLevel::Trace);
    Server::new("0.0.0.0:8080")?
        .route(Method::GET, "/api/v1/inc/:id", |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .assets_path("./examples/html/assets")
        .run()?;

    Ok(())
}
