use super::Shutdown;
use std::time::{Duration, Instant};

#[test]
fn test_poll_timeout_bounds() {
    // Pure function: no signal handlers or pipes involved, safe to run
    // concurrently with the server integration tests.
    assert_eq!(
        Shutdown::poll_timeout(None, Duration::from_secs(30)),
        1_000
    );

    let remaining = Shutdown::poll_timeout(
        Some(Instant::now() + Duration::from_millis(50)),
        Duration::from_secs(30),
    );
    assert!((40..=50).contains(&remaining), "unexpected timeout: {remaining}");

    let expired = Shutdown::poll_timeout(Some(Instant::now()), Duration::from_secs(30));
    assert_eq!(expired, 1, "expired deadline must still yield a positive timeout");
}
