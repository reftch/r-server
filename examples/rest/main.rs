use r_server::{
    response::{self, Status},
    router::Method,
    server::Server,
    utils::get_env,
};

const PORT: u16 = 8080;

fn main() -> std::io::Result<()> {
    let host = get_env("HOST", "0.0.0.0".to_string());
    let port = get_env("PORT", PORT);
    let addr = format!("{}:{}", host, port);

    let mut server = Server::new(&addr)?;

    server.add_route(Method::GET, "/api/v1/inc/:id", |req, res| {
        if let Some(id) = req.param("id") {
            if let Ok(val) = id.parse::<i32>() {
                res.set_content_type(response::ContentType::JSON)
                    .set_body(format!("{{\"value\":{}}}", val + 1));
            } else {
                res.set_status(Status::BadRequest)
                    .set_body("Invalid ID".to_string());
            }
        }
    });

    server.run()?;

    Ok(())
}
