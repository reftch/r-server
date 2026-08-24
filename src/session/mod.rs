//! Server-side browser sessions.
//!
//! A [`SessionStore`] owns all live sessions and is shared across worker
//! threads. For each request the connection layer resolves the session from
//! the `Cookie` header (or mints a fresh one) and attaches a cheap
//! [`Session`] handle to the [`Request`](crate::request::Request); handlers
//! read and mutate it through interior mutability.
//!
//! ```ignore
//! fn handler(req: &Request, res: &mut Response) {
//!     if let Some(session) = req.session() {
//!         session.set("visits", "42");
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cookie name carrying the session identifier.
pub const SESSION_COOKIE: &str = "SID";

/// Seconds between background sweeps of expired sessions.
const SWEEP_INTERVAL_SECS: u64 = 30;

/// Raw random bytes per session id (rendered as 2x hex characters).
const SID_BYTES: usize = 16;

/// Session state owned by the store and shared through [`Session`].
pub struct SessionInner {
    /// Opaque random identifier stored in the browser cookie.
    pub session_id: String,
    /// Unix timestamp (seconds) of session creation.
    pub created_at: u64,
    /// Unix timestamp (seconds) of the last request seen for this session.
    pub last_activity: u64,
    /// Arbitrary application data.
    pub data: HashMap<String, String>,
    destroyed: bool,
}

/// Cloneable handle to one browser's session.
///
/// Cloning is cheap (an atomic refcount bump); every clone observes the same
/// underlying [`SessionInner`]. Methods take `&self` because handlers only
/// receive an immutable request reference.
#[derive(Clone)]
pub struct Session(Arc<Mutex<SessionInner>>);

impl Session {
    pub(crate) fn new(inner: SessionInner) -> Self {
        Self(Arc::new(Mutex::new(inner)))
    }

    /// The opaque session identifier sent in the cookie.
    pub fn id(&self) -> String {
        lock(&self.0).session_id.clone()
    }

    /// Reads a value from the session data.
    pub fn get(&self, key: &str) -> Option<String> {
        lock(&self.0).data.get(key).cloned()
    }

    /// Writes a value into the session data.
    pub fn set(&self, key: impl Into<String>, value: impl Into<String>) {
        lock(&self.0).data.insert(key.into(), value.into());
    }

    /// Removes a value from the session data.
    pub fn remove(&self, key: &str) -> Option<String> {
        lock(&self.0).data.remove(key)
    }

    /// Marks the session as ended; the store evicts it after the current
    /// request and the browser cookie is expired.
    pub fn destroy(&self) {
        lock(&self.0).destroyed = true;
    }

    /// Whether [`Session::destroy`] was called during this request.
    pub fn is_destroyed(&self) -> bool {
        lock(&self.0).destroyed
    }

    pub(crate) fn touch(&self, now: u64) {
        lock(&self.0).last_activity = now;
    }

    pub(crate) fn idle_secs(&self, now: u64) -> u64 {
        now.saturating_sub(lock(&self.0).last_activity)
    }
}

/// Thread-safe container of live sessions shared by every worker thread.
pub struct SessionStore {
    sessions: Mutex<HashMap<String, Session>>,
    ttl_secs: u64,
    next_sweep: AtomicU64,
}

impl SessionStore {
    /// Creates a store dropping sessions idle longer than `ttl_secs`.
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            ttl_secs,
            next_sweep: AtomicU64::new(0),
        }
    }

    /// The configured idle timeout in seconds.
    pub fn ttl(&self) -> u64 {
        self.ttl_secs
    }

    /// Number of sessions currently tracked.
    pub fn len(&self) -> usize {
        lock(&self.sessions).len()
    }

    /// Whether no sessions are tracked.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Resolves the session for an incoming request.
    ///
    /// Returns the handle plus whether a fresh session was created; when the
    /// caller passes a cookie that is unknown or already expired, a new
    /// session is minted and the stale entry dropped.
    pub fn get_or_create(&self, sid: Option<&str>) -> (Session, bool) {
        let now = now_secs();
        self.sweep_if_due(now);

        if let Some(sid) = sid {
            let mut map = lock(&self.sessions);

            if let Some(existing) = map.get(sid) {
                // Not yet idle long enough to expire: refresh activity.
                if existing.idle_secs(now) < self.ttl_secs {
                    existing.touch(now);
                    return (existing.clone(), false);
                }
            }

            // Unknown or expired cookie: drop the stale entry, mint fresh.
            map.remove(sid);
        }

        let inner = SessionInner {
            session_id: generate_sid(),
            created_at: now,
            last_activity: now,
            data: HashMap::new(),
            destroyed: false,
        };

        let session = Session::new(inner);
        lock(&self.sessions).insert(session.id(), session.clone());

        (session, true)
    }

    /// Evicts a session immediately (e.g. after logout).
    pub fn destroy(&self, sid: &str) {
        lock(&self.sessions).remove(sid);
    }

    fn sweep_if_due(&self, now: u64) {
        if now < self.next_sweep.load(Ordering::Relaxed) {
            return;
        }

        // Schedule the next attempt even if several threads race here; the
        // sweep itself is idempotent.
        self.next_sweep
            .store(now + SWEEP_INTERVAL_SECS, Ordering::Relaxed);

        lock(&self.sessions).retain(|_, session| session.idle_secs(now) < self.ttl_secs);
    }
}

/// Extracts the session id from a raw `Cookie` header value such as
/// `"theme=dark; SID=abc123"`. Cookie names are case-sensitive per RFC 6265.
pub fn sid_from_cookie(cookie_header: &str) -> Option<&str> {
    cookie_header.split(';').find_map(|pair| {
        let (name, value) = pair.trim().split_once('=')?;
        (name.trim() == SESSION_COOKIE)
            .then_some(value.trim())
            .filter(|value| !value.is_empty())
    })
}

/// `Set-Cookie` header value establishing the session cookie.
pub fn session_set_cookie(sid: &str, max_age_secs: u64) -> String {
    format!("{SESSION_COOKIE}={sid}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age_secs}")
}

/// `Set-Cookie` header value expiring the session cookie in the browser.
pub fn cleared_session_cookie() -> String {
    format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Sessions are best-effort shared state; recover from poisoning instead
    // of propagating a panic from an unrelated handler to every request that
    // touches the same store.
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since_epoch| since_epoch.as_secs())
        .unwrap_or_default()
}

fn generate_sid() -> String {
    let mut bytes = [0u8; SID_BYTES];
    openssl::rand::rand_bytes(&mut bytes).expect("OpenSSL RNG must be available");

    let mut sid = String::with_capacity(SID_BYTES * 2);
    for byte in bytes {
        // Writing two hex chars into a String never fails.
        write!(sid, "{byte:02x}").unwrap();
    }

    sid
}

#[cfg(test)]
mod tests;
