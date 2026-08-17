use r_server::{response, router::Method, server::http::Server};

fn main() -> std::io::Result<()> {
    Server::new()?
        .route(Method::GET, "/api/v1/users/:id", |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .run()?;

    Ok(())
}
