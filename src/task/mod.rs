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
    core::metadata::Metadata,
    response::{ContentType, Response, Status},
};

type Cancel = Arc<AtomicBool>;

static TASKS: OnceLock<Mutex<HashMap<String, Cancel>>> = OnceLock::new();

pub fn repeat_every<F>(key: impl Into<String>, metadata: &dyn Metadata, delay: Duration, mut f: F)
where
    F: FnMut(&mut Response) + Send + 'static,
{
    // Clone the metadata box before moving into the background thread
    let conn = metadata.clone_box();
    let tasks = TASKS.get_or_init(|| Mutex::new(HashMap::new()));

    let key = key.into();
    let cancel = Arc::new(AtomicBool::new(false));

    if let Some(old_cancel) = tasks.lock().unwrap().insert(key, cancel.clone()) {
        old_cancel.store(true, Ordering::Relaxed);
    }

    thread::spawn(move || {
        while !cancel.load(Ordering::Relaxed) {
            // Clone the Box<dyn Metadata> for each iteration
            let mut response =
                Response::new(Arc::from(conn.clone()), Status::Ok, b"", ContentType::SSE);

            f(&mut response);

            thread::sleep(delay);
        }
    });
}

pub fn once<F>(metadata: &dyn Metadata, mut f: F)
where
    F: FnMut(&mut Response) + Send + 'static,
{
    let conn = metadata.clone_box();

    thread::spawn(move || {
        let mut response =
            Response::new(Arc::from(conn.clone()), Status::Ok, b"", ContentType::TEXT);
        f(&mut response);
    });
}
