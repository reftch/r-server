use r_server::{core::http::Server, response, router::Method};

fn main() -> std::io::Result<()> {
    Server::new("127.0.0.1:8080")?
        .route(Method::GET, "/api/v1/users/:id", |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .run()?;

    Ok(())
}
