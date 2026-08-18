use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use crate::{
    response::{ContentType, Response, Status},
    server::metadata::Metadata,
};

type Cancel = Arc<AtomicBool>;

static TASKS: OnceLock<Mutex<HashMap<String, Cancel>>> = OnceLock::new();

pub fn repeat_every<F>(key: impl Into<String>, metadata: &dyn Metadata, delay: Duration, mut f: F)
where
    F: for<'a> FnMut(&Response<'a>) + Send + 'static,
{
    let conn = metadata
        .try_clone_metadata()
        .expect("failed to clone connection metadata");
    let tasks = TASKS.get_or_init(|| Mutex::new(HashMap::new()));

    let key = key.into();
    let cancel = Arc::new(AtomicBool::new(false));

    if let Some(old_cancel) = tasks.lock().unwrap().insert(key, cancel.clone()) {
        old_cancel.store(true, Ordering::Relaxed);
    }

    thread::spawn(move || {
        while !cancel.load(Ordering::Relaxed) {
            let response = Response::new(conn.as_ref(), Status::Ok, b"", ContentType::SSE);

            f(&response);

            thread::sleep(delay);
        }
    });
}

pub fn once<F>(conn: Box<dyn Metadata>, mut f: F)
where
    F: for<'a> FnMut(&mut Response<'a>) + Send + 'static,
{
    thread::spawn(move || {
        let mut response = Response::new(conn.as_ref(), Status::Ok, b"", ContentType::TEXT);

        f(&mut response);
    });
}
