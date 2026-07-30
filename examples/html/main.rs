use r_server::{response, router::Method, server::HttpServer};

fn main() -> std::io::Result<()> {
    // r_server::logger::set_level(logger::LogLevel::Trace);
    HttpServer::new("0.0.0.0:8082")?
        .route(Method::GET, "/api/v1/users/:id", |req, res| {
            if let Some(id) = req.param("id") {
                res.send("Hello".to_string());
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .assets_path("./examples/html/assets")
        .run()?;

    Ok(())
}
