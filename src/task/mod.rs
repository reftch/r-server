use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{
    core::metadata::Metadata,
    response::{ContentType, Response, Status},
};

type Cancel = Arc<AtomicBool>;

/// A registered repeating background task: its cancellation flag plus the
/// thread handle needed to await it during shutdown.
struct BackgroundTask {
    cancel: Cancel,
    handle: JoinHandle<()>,
}

static TASKS: OnceLock<Mutex<HashMap<String, BackgroundTask>>> = OnceLock::new();

/// Granularity of the cancellation check while a repeating task sleeps.
const CANCEL_TICK: Duration = Duration::from_millis(50);

pub fn repeat_every<F>(key: impl Into<String>, metadata: &dyn Metadata, delay: Duration, mut f: F)
where
    F: FnMut(&mut Response) + Send + 'static,
{
    // Clone the metadata box before moving into the background thread
    let conn = metadata.clone_box();
    let tasks = TASKS.get_or_init(|| Mutex::new(HashMap::new()));

    let key = key.into();
    let cancel = Arc::new(AtomicBool::new(false));
    let task_cancel = Arc::clone(&cancel);

    let handle = thread::spawn(move || {
        while !task_cancel.load(Ordering::Relaxed) {
            // Clone the Box<dyn Metadata> for each iteration
            let mut response =
                Response::new(Arc::from(conn.clone()), Status::Ok, b"", ContentType::SSE);

            f(&mut response);

            // Sleep in short slices so cancellation stays responsive even
            // when `delay` is long.
            let mut remaining = delay;
            while !remaining.is_zero() && !task_cancel.load(Ordering::Relaxed) {
                let tick = remaining.min(CANCEL_TICK);
                thread::sleep(tick);
                remaining -= tick;
            }
        }
    });

    if let Some(old_task) = tasks
        .lock()
        .unwrap()
        .insert(key, BackgroundTask { cancel, handle })
    {
        old_task.cancel.store(true, Ordering::Relaxed);
        // The replaced thread detaches and exits on its next tick.
    }
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

/// Cancels every repeating task and waits for its thread to exit. Called by
/// the server's `run()` once the event loops have drained.
pub(crate) fn cancel_all_and_join() {
    let tasks = TASKS.get_or_init(|| Mutex::new(HashMap::new()));

    for (_, task) in tasks.lock().unwrap().drain() {
        task.cancel.store(true, Ordering::Relaxed);
        let _ = task.handle.join();
    }
}
