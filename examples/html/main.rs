use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use r_server::{
    response::{self, ContentType, Response, Status},
    router::Method,
    server::{connection::ConnectionMetadata, http::Server},
};

type Cancel = Arc<AtomicBool>;

static TASKS: OnceLock<Mutex<HashMap<String, Cancel>>> = OnceLock::new();

// ============================================================
// Generic stream cloning
// ============================================================

trait StreamClone: Send + 'static {
    type Error;

    fn try_clone(&self) -> Result<Self, Self::Error>
    where
        Self: Sized;
}

// ============================================================
// TCP
// ============================================================

impl StreamClone for std::net::TcpStream {
    type Error = std::io::Error;

    fn try_clone(&self) -> Result<Self, Self::Error> {
        std::net::TcpStream::try_clone(self)
    }
}

fn repeat_every<S, F>(key: &str, stream: S, delay: Duration, mut f: F)
where
    S: StreamClone,
    S::Error: Debug,
    F: FnMut(&Response<S>) + Send + 'static,
{
    let tasks = TASKS.get_or_init(|| Mutex::new(HashMap::new()));
    let cancel = Arc::new(AtomicBool::new(false));

    // Cancel the previous task with the same key.
    if let Some(old_cancel) = tasks
        .lock()
        .unwrap()
        .insert(key.to_string(), cancel.clone())
    {
        old_cancel.store(true, Ordering::Relaxed);
    }

    thread::spawn(move || {
        while !cancel.load(Ordering::Relaxed) {
            let conn = ConnectionMetadata {
                stream: stream.try_clone().expect("Error cloning stream"),
            };
            let response = Response::new(&conn, Status::Ok, b"", ContentType::SSE);

            f(&response);

            thread::sleep(delay);
        }
    });
}

fn main() -> std::io::Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    // r_server::logger::set_level(logger::LogLevel::Trace);
    Server::new("0.0.0.0:8082")?
        .route(Method::GET, "/api/v1/users/:id", move |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .route(Method::GET, "/stream", move |_, res| {
            res.content_type(response::ContentType::SSE);
            let stream = res.metadata.stream.try_clone().unwrap();

            repeat_every("stream", stream, Duration::from_secs(1), move |response| {
                let _ = response.sse(&format!(
                    "data: {}\n\n",
                    COUNTER.fetch_add(1, Ordering::Relaxed) + 1
                ));
            });
        })
        .assets_path("./examples/html/assets")
        .run()?;

    Ok(())
}
