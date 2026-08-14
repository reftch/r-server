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
    core::connection::{ConnectionMetadata, ConnectionStreamClone},
    response::{ContentType, Response, Status},
};

type Cancel = Arc<AtomicBool>;

static TASKS: OnceLock<Mutex<HashMap<String, Cancel>>> = OnceLock::new();

pub fn repeat_every<M, F>(key: &str, conn: ConnectionMetadata<M>, delay: Duration, mut f: F)
where
    M: ConnectionStreamClone + Send + 'static,
    F: FnMut(&Response<'_, M>) + Send + 'static,
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
            let response = Response::new(&conn, Status::Ok, b"", ContentType::SSE);

            f(&response);

            thread::sleep(delay);
        }
    });
}

pub fn once<M, F>(conn: ConnectionMetadata<M>, mut f: F)
where
    M: ConnectionStreamClone + Send + 'static,
    F: FnMut(&mut Response<'_, M>) + Send + 'static,
{
    thread::spawn(move || {
        let mut response = Response::new(&conn, Status::Ok, b"", ContentType::TEXT);
        f(&mut response);
    });
}
