use std::io;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Byte written to the self-pipe to interrupt `poll()` in every worker.
const WAKE_SIGNAL: u8 = 1;

/// Set by the signal handler (or [`Shutdown::trigger`]) when a graceful
/// shutdown has been requested. Observed by every worker event loop of
/// every active server.
static TRIGGERED: AtomicBool = AtomicBool::new(false);

/// Write end of the latest self-pipe; `-1` while nothing is installed.
static WAKE_WRITE_FD: AtomicI32 = AtomicI32::new(-1);

/// Installation generation. Only the latest coordinator restores signal
/// dispositions on drop, so a lingering older server cannot disarm the
/// handlers of the currently active one.
static GENERATION: AtomicU64 = AtomicU64::new(0);

extern "C" fn on_signal(_signum: libc::c_int) {
    // Async-signal-safe operations only: an atomic store plus a single
    // write(2) to wake any workers blocked in poll().
    TRIGGERED.store(true, Ordering::SeqCst);

    let fd = WAKE_WRITE_FD.load(Ordering::SeqCst);
    if fd >= 0 {
        let byte = [WAKE_SIGNAL];
        unsafe {
            libc::write(fd, byte.as_ptr().cast(), 1);
        }
    }
}

fn set_nonblocking_cloexec(fd: i32) -> io::Result<()> {
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(io::Error::last_os_error());
        }

        let flags = libc::fcntl(fd, libc::F_GETFD);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) < 0 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}

/// Coordinates a graceful shutdown across all worker event loops.
///
/// Workers watch the shared triggered flag and a self-pipe registered in
/// their poll set, so a signal wakes `poll()` immediately instead of waiting
/// for its periodic timeout. `SIGINT` and `SIGTERM` feed the same path via
/// handlers installed by [`Shutdown::install`].
///
/// Installing a new coordinator takes over from any previous one: signals
/// shut down every active server, but only the latest installation owns the
/// process-wide disposition and restores it on drop.
pub(crate) struct Shutdown {
    wake_read_fd: i32,
    wake_write_fd: i32,
    generation: u64,
}

impl Shutdown {
    /// Creates the self-pipe and installs `SIGINT`/`SIGTERM` handlers,
    /// taking over from any previously installed coordinator.
    pub fn install() -> io::Result<Self> {
        let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

        // Retire the previous wake pipe, if any. Its owner notices the
        // changed global at drop time and skips closing this end.
        let prev = WAKE_WRITE_FD.swap(-1, Ordering::SeqCst);
        if prev >= 0 {
            unsafe {
                libc::close(prev);
            }
        }

        let mut fds = [-1 as libc::c_int; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }

        for fd in fds {
            if let Err(err) = set_nonblocking_cloexec(fd) {
                unsafe {
                    libc::close(fds[0]);
                    libc::close(fds[1]);
                }
                return Err(err);
            }
        }

        WAKE_WRITE_FD.store(fds[1], Ordering::SeqCst);

        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = on_signal as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            // poll(2) still returns EINTR even with SA_RESTART; other
            // in-flight syscalls restart instead of surfacing errors.
            action.sa_flags = libc::SA_RESTART;

            if libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut()) != 0
                || libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut()) != 0
            {
                let err = io::Error::last_os_error();
                WAKE_WRITE_FD.store(-1, Ordering::SeqCst);
                libc::close(fds[0]);
                libc::close(fds[1]);
                return Err(err);
            }
        }

        Ok(Self {
            wake_read_fd: fds[0],
            wake_write_fd: fds[1],
            generation,
        })
    }

    /// File descriptor to include in the worker poll set; a shutdown
    /// wake-up arrives as readable data here.
    pub fn wake_fd(&self) -> i32 {
        self.wake_read_fd
    }

    /// Reads pending wake bytes so level-triggered poll readiness clears.
    pub fn drain_wake_pipe(&self) {
        let mut buf = [0u8; 64];
        loop {
            let n = unsafe { libc::read(self.wake_read_fd, buf.as_mut_ptr().cast(), buf.len()) };
            if n <= 0 {
                break;
            }
        }
    }

    /// Whether a graceful shutdown has been requested.
    pub fn is_triggered(&self) -> bool {
        TRIGGERED.load(Ordering::SeqCst)
    }

    /// Requests a graceful shutdown programmatically; equivalent to
    /// receiving `SIGINT`/`SIGTERM`.
    pub fn trigger(&self) {
        TRIGGERED.store(true, Ordering::SeqCst);

        let fd = WAKE_WRITE_FD.load(Ordering::SeqCst);
        if fd >= 0 {
            let byte = [WAKE_SIGNAL];
            unsafe {
                libc::write(fd, byte.as_ptr().cast(), 1);
            }
        }
    }

    /// Remaining time until the drain deadline passes.
    ///
    /// Returns `fallback` when no deadline is set, clamped to at most one
    /// second so the triggered flag is polled regularly regardless.
    pub fn poll_timeout(deadline: Option<Instant>, fallback: Duration) -> i32 {
        const CAP_MS: u128 = 1_000;

        let millis = match deadline {
            Some(deadline) => deadline.saturating_duration_since(Instant::now()).as_millis(),
            None => fallback.as_millis(),
        };

        millis.clamp(1, CAP_MS) as i32
    }

    fn uninstall(&self) {
        // Close our write end only while the global still points at it; a
        // newer installation has already closed it for us otherwise.
        if WAKE_WRITE_FD
            .compare_exchange(
                self.wake_write_fd,
                -1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
        {
            unsafe {
                libc::close(self.wake_write_fd);
            }
        }

        unsafe {
            libc::close(self.wake_read_fd);
        }

        // Only the latest installation may restore dispositions; an older
        // coordinator dropping late must not disarm the active one.
        if GENERATION.load(Ordering::SeqCst) == self.generation {
            unsafe {
                let default_action: libc::sigaction = std::mem::zeroed();
                libc::sigaction(libc::SIGINT, &default_action, std::ptr::null_mut());
                libc::sigaction(libc::SIGTERM, &default_action, std::ptr::null_mut());
            }

            TRIGGERED.store(false, Ordering::SeqCst);
        }
    }
}

impl Drop for Shutdown {
    fn drop(&mut self) {
        self.uninstall();
    }
}

#[cfg(test)]
mod tests;
