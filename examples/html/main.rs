use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use r_server::{
    response::{self, ContentType, Response, Status},
    router::Method,
    server::{connection::ConnectionMetadata, http::Server},
};

// fn repeat_every<F, T>(key: String, stream: T, delay: Duration, mut f: F)
// where
//     F: FnMut(&T) + Send + 'static,
//     T: Send + 'static,
// {
//     // Stop previous task for this key.
//     if let Some(old_cancel) = tasks.lock().unwrap().insert(key.clone(), cancel.clone()) {
//         old_cancel.store(true, Ordering::Relaxed);
//     }

//     thread::spawn(move || {
//         loop {
//             f(&stream);
//             thread::sleep(delay);
//         }
//     });
// }

type Cancel = Arc<AtomicBool>;

static TASKS: OnceLock<Mutex<HashMap<String, Cancel>>> = OnceLock::new();

fn repeat_every<F, T>(key: &str, stream: T, delay: Duration, mut f: F)
where
    F: FnMut(&T) + Send + 'static,
    T: Send + 'static,
{
    let tasks = TASKS.get_or_init(|| Mutex::new(HashMap::new()));

    let cancel = Arc::new(AtomicBool::new(false));

    if let Some(old_cancel) = tasks
        .lock()
        .unwrap()
        .insert(key.to_string(), cancel.clone())
    {
        old_cancel.store(true, Ordering::Relaxed);
    }

    thread::spawn(move || {
        while !cancel.load(Ordering::Relaxed) {
            f(&stream);
            thread::sleep(delay);
        }
    });
}

fn main() -> std::io::Result<()> {
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
            let stream = res.metadata.stream.try_clone().expect("Error cloning");

            let mut i = 0;
            repeat_every("stream", stream, Duration::from_secs(1), move |stream| {
                i += 1;
                let conn = ConnectionMetadata {
                    stream: stream.try_clone().expect("Error cloning"),
                };
                let response = Response::new(&conn, Status::Ok, b"", ContentType::SSE);

                let message = format!("data: {}\n\n", i);
                let _ = response.sse(&message);
            });
        })
        // .assets_path("./examples/html/assets")
        .run()?;

    Ok(())
}
