use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use crate::{
    response::{ContentType, Response, Status},
    server::connection::ConnectionMetadata,
};
type Cancel = Arc<AtomicBool>;

static TASKS: OnceLock<Mutex<HashMap<String, Cancel>>> = OnceLock::new();

// ============================================================
// Generic stream cloning
// ============================================================

pub trait StreamClone: Send + 'static {
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

pub fn repeat_every<S, F>(key: &str, stream: S, delay: Duration, mut f: F)
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
