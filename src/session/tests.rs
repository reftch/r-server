use std::sync::atomic::Ordering;

use super::*;

fn is_hex_sid(sid: &str) -> bool {
    sid.len() == SID_BYTES * 2
        && sid
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[test]
fn creates_new_session_without_cookie() {
    let store = SessionStore::new(3600);

    let before = now_secs();
    let (session, is_new) = store.get_or_create(None);

    assert!(is_new);
    assert!(is_hex_sid(&session.id()));
    assert_eq!(store.len(), 1);

    let inner = lock(&session.0);
    // The second boundary may tick during creation; accept both seconds.
    assert!(inner.created_at >= before.saturating_sub(1));
    assert!(inner.created_at <= now_secs());
    assert_eq!(inner.last_activity, inner.created_at);
    assert!(inner.data.is_empty());
}

#[test]
fn reuses_existing_session_for_same_sid() {
    let store = SessionStore::new(3600);
    let (first, _) = store.get_or_create(None);
    first.set("cart", "eggs");

    let (second, is_new) = store.get_or_create(Some(&first.id()));

    assert!(!is_new);
    assert_eq!(second.id(), first.id());
    assert_eq!(second.get("cart"), Some("eggs".into()));
    assert_eq!(store.len(), 1);
}

#[test]
fn clones_share_state() {
    let (session, _) = SessionStore::new(3600).get_or_create(None);
    let clone = session.clone();

    clone.set("user_id", "u1");

    assert_eq!(session.get("user_id"), Some("u1".into()));

    assert_eq!(clone.remove("user_id"), Some("u1".into()));
    assert_eq!(session.get("user_id"), None);
}

#[test]
fn unknown_sid_creates_fresh_session() {
    let store = SessionStore::new(3600);
    let (existing, _) = store.get_or_create(None);

    let (fresh, is_new) = store.get_or_create(Some("deadbeef"));

    assert!(is_new);
    assert_ne!(fresh.id(), existing.id());
    assert_eq!(store.len(), 2);
}

#[test]
fn expired_session_is_recreated_and_evicted() {
    // ttl of zero expires every lookup on a later second boundary.
    let store = SessionStore::new(0);
    let (expired, _) = store.get_or_create(None);

    let (fresh, is_new) = store.get_or_create(Some(&expired.id()));

    assert!(is_new);
    assert_ne!(fresh.id(), expired.id());
    // The stale entry was dropped eagerly.
    assert_eq!(store.len(), 1);
}

#[test]
fn destroy_marks_session_and_store_evicts() {
    let store = SessionStore::new(3600);
    let (session, _) = store.get_or_create(None);

    assert!(!session.is_destroyed());
    session.destroy();
    assert!(session.is_destroyed());

    store.destroy(&session.id());
    assert!(store.is_empty());
}

#[test]
fn sweep_drops_idle_sessions() {
    let store = SessionStore::new(10);
    let (stale, _) = store.get_or_create(None);

    // Backdate the session past the TTL and force the sweep to run.
    lock(&stale.0).last_activity = now_secs().saturating_sub(60);
    store.next_sweep.store(0, Ordering::Relaxed);

    let (_fresh, is_new) = store.get_or_create(None);

    assert!(is_new);
    assert_eq!(store.len(), 1);
}

#[test]
fn infinite_sessions_never_expire_on_lookup() {
    let store = SessionStore::infinite();
    let (session, _) = store.get_or_create(None);

    // Backdate far beyond any finite TTL; an infinite store must still reuse.
    lock(&session.0).last_activity = now_secs().saturating_sub(SWEEP_INTERVAL_SECS * 1_000);

    let (reused, is_new) = store.get_or_create(Some(&session.id()));

    assert!(!is_new);
    assert_eq!(reused.id(), session.id());
}

#[test]
fn infinite_sessions_survive_sweep() {
    let store = SessionStore::infinite();
    let (session, _) = store.get_or_create(None);
    lock(&session.0).last_activity = now_secs().saturating_sub(3_600);

    store.next_sweep.store(0, Ordering::Relaxed);
    let (_, is_new) = store.get_or_create(Some(&session.id()));

    assert!(!is_new);
    assert_eq!(store.len(), 1);
    assert_eq!(store.ttl(), None);
}

#[test]
fn recent_sessions_survive_sweep() {
    let store = SessionStore::new(3600);
    let (session, _) = store.get_or_create(None);

    store.next_sweep.store(0, Ordering::Relaxed);
    store.get_or_create(None);

    assert!(!store.is_empty());
    let (_, reused) = store.get_or_create(Some(&session.id()));
    assert!(!reused);
}

#[test]
fn parses_sid_from_cookie_header() {
    assert_eq!(
        sid_from_cookie("theme=dark; SID=abc123; lang=en"),
        Some("abc123")
    );
    assert_eq!(sid_from_cookie("SID=abc"), Some("abc"));
    assert_eq!(sid_from_cookie("A=1;SID=no-space"), Some("no-space"));
    assert_eq!(sid_from_cookie(" SID = padded "), Some("padded"));
}

#[test]
fn ignores_missing_or_foreign_cookies() {
    assert_eq!(sid_from_cookie("theme=dark"), None);
    assert_eq!(sid_from_cookie(""), None);
    // Cookie names are case-sensitive per RFC 6265.
    assert_eq!(sid_from_cookie("sid=lowercase"), None);
    // Suffix matches must not win.
    assert_eq!(sid_from_cookie("OTHERSID=x"), None);
    // Empty values are treated as absent.
    assert_eq!(sid_from_cookie("SID="), None);
}

#[test]
fn builds_set_cookie_headers() {
    assert_eq!(
        session_set_cookie("abc", Some(3600)),
        "SID=abc; Path=/; HttpOnly; SameSite=Lax; Max-Age=3600"
    );
    // Infinite sessions omit Max-Age: browser-session cookie.
    assert_eq!(
        session_set_cookie("abc", None),
        "SID=abc; Path=/; HttpOnly; SameSite=Lax"
    );
    assert_eq!(
        cleared_session_cookie(),
        "SID=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
    );
}
