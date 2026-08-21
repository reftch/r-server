use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use r_server::{router::Method, server::http::Server, task};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn main() -> std::io::Result<()> {
    Server::new()?
        .route(Method::GET, "/stream", |req, res| {
            task::repeat_every(
                req.path.to_string(), // Converts Box<str> / &str to String
                &*res.metadata,       // Dereferences Arc<dyn Metadata> to &dyn Metadata
                Duration::from_millis(50),
                |res| {
                    let _ = res.stream(&format!(
                        "{}\n\n",
                        COUNTER.fetch_add(1, Ordering::Relaxed) + 1
                    ));
                },
            );
        })
        .bind("0.0.0.0", 8080)
        .assets_path("./examples/static/assets")
        .run()?;
    Ok(())
}
