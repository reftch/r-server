use r_server::{
    core::http::Server,
    info,
    request::Request,
    response::{self, Response},
    router::{Method, Next},
};

fn logger(req: &Request, res: &mut Response, next: Next) {
    let start = std::time::Instant::now();

    next.run(req, res);

    info!("{} {} - {:?}", req.method, req.path, start.elapsed());
}

fn hello_handler(_req: &Request, res: &mut Response) {
    res.body("Hello, World!");
}

fn main() -> std::io::Result<()> {
    Server::new()?
        .use_middleware(logger)
        .route(Method::GET, "/hello", hello_handler)
        .route(Method::GET, "/api/v1/users/:id", |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .bind("0.0.0.0", 8082)
        .run()?;

    Ok(())
}
