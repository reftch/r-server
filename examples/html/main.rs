use r_server::{
    info, logger,
    request::Request,
    response::{self, Response},
    router::{HandlerFn, Method, MiddlewareFn, Router},
    server::http::Server,
    task,
};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn logger<'a>(
    req: &Request<'a>,
    res: &mut Response<'a>,
    next_mws: &[MiddlewareFn],
    handler: HandlerFn,
) {
    let start = std::time::Instant::now();

    // Now lifetimes align exactly with Router::next
    Router::next(req, res, next_mws, handler);

    info!("{} {} - {:?}", req.method, req.path, start.elapsed());
}

fn main() -> std::io::Result<()> {
    r_server::logger::set_level(logger::LogLevel::Info);

    Server::new()?
        .use_middleware(MiddlewareFn(logger))
        .route(Method::GET, "/api/v1/users/:id", |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .route(Method::GET, "/stream", |req, res| {
            task::repeat_every(
                req.path.to_owned(),
                res.metadata,
                Duration::from_millis(50),
                |res| {
                    let _ = res.stream(&format!(
                        "{}\n\n",
                        COUNTER.fetch_add(1, Ordering::Relaxed) + 1
                    ));
                },
            );
        })
        .assets_path("./examples/html/assets")
        .run()?;

    Ok(())
}
