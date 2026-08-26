use r_server::{
    core::http::Server,
    request::Request,
    response::{self, Response},
    router::Method,
};

fn hello_handler(_req: &Request, res: &mut Response) {
    res.body("Hello, World!");
}

fn main() -> std::io::Result<()> {
    Server::new()?
        .route(Method::GET, "/hello", hello_handler)
        .route(Method::GET, "/api/v1/users/:id", |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .workers(2)
        .run()
}
