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
