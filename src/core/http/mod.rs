use libc::{POLLERR, POLLHUP, POLLIN, POLLOUT};
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::core::connection::{ConnectionMetadata, ConnectionStreamClone};
use crate::core::metadata::Metadata;
use crate::core::shutdown::Shutdown;
use crate::core::{POLL_TIMEOUT, STATIC_DIRECTORY};
use crate::request::Request;
use crate::request::session::{
    SessionStore, cleared_session_cookie, session_set_cookie, sid_from_cookie,
};
use crate::response::{ContentType, Response, Status};
use crate::router::Next;
use crate::router::Router;
use crate::task;
use crate::utils::get_file_info;
use crate::{debug, error, info, trace};
use std::io::{self, Read, Write};

use std::sync::Arc;
use std::thread;

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

pub struct Connection<S> {
    pub metadata: ConnectionMetadata<S>,
    pub read_buf: Vec<u8>,
    pub write_buf: Vec<u8>,
}

impl Connection<TcpStream> {
    pub fn new(socket: TcpStream) -> io::Result<Self> {
        socket.set_nonblocking(true)?;
        Ok(Self {
            metadata: ConnectionMetadata {
                stream: Arc::new(socket),
            },
            read_buf: Vec::with_capacity(1024),
            write_buf: Vec::new(),
        })
    }
}

impl ConnectionStreamClone for TcpStream {
    fn clone_stream(&self) -> io::Result<Self> {
        self.try_clone()
    }
}

enum WriteState {
    Continue,
    Done,
    Close,
}

/// Immutable state shared by all worker threads.
struct WorkerCtx {
    router: Arc<Router>,
    assets_path: PathBuf,
    sessions: Option<Arc<SessionStore>>,
}

pub struct Server {
    init_start: Instant,
    router: Arc<Router>,
    assets_path: PathBuf,
    sessions: Option<Arc<SessionStore>>,
    addr: String,
    workers: usize,
    shutdown_timeout: Duration,
}

impl Server {
    pub fn new() -> io::Result<Self> {
        let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

        let port: u16 = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(8080);

        let addr = format!("{}:{}", host, port);

        Self::new_with_assets(&addr, PathBuf::from(STATIC_DIRECTORY))
    }

    pub fn bind(&mut self, host: &str, port: u16) -> &mut Self {
        self.addr = format!("{}:{}", host, port);
        self
    }

    pub fn assets_path(&mut self, path: &str) -> &mut Self {
        self.assets_path = PathBuf::from(path);
        self
    }

    /// Sets the number of worker threads. Each worker runs an independent
    /// event loop with its own connection set; the OS distributes incoming
    /// connections across workers (via `SO_REUSEPORT` on Linux, a shared
    /// listening socket elsewhere). Defaults to 1.
    pub fn workers(&mut self, n: usize) -> &mut Self {
        self.workers = n.max(1);
        self
    }

    /// Sets the maximum duration to wait for in-flight responses to flush
    /// when a graceful shutdown is triggered (SIGINT, SIGTERM). Idle
    /// keep-alive connections are closed immediately; connections with a
    /// pending response are flushed and closed; anything still busy when the
    /// timeout elapses is force-closed. Defaults to 30 seconds.
    pub fn shutdown_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.shutdown_timeout = timeout;
        self
    }

    /// Enables server-side browser sessions with the given idle timeout.
    ///
    /// Every parsed request receives a session handle at `request.session()`;
    /// sessions are resolved from the `Cookie` header or minted fresh, and a
    /// new session is announced to the browser via a `Set-Cookie` header on
    /// the response. Sessions idle for more than `ttl_secs` are dropped from
    /// the store; a negative value such as `-1` means sessions never expire.
    /// Disabled by default. See [`crate::request::session`].
    pub fn sessions_ttl(&mut self, ttl_secs: i64) -> &mut Self {
        self.sessions = Some(Arc::new(if ttl_secs < 0 {
            SessionStore::infinite()
        } else {
            SessionStore::new(ttl_secs as u64)
        }));
        self
    }

    fn new_with_assets(addr: &str, assets_path: PathBuf) -> io::Result<Self> {
        let init_start = Instant::now();
        let router = Arc::new(Router::new());

        // Validate the address, but DO NOT bind the listener yet.
        addr.parse::<std::net::SocketAddr>()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        Ok(Server {
            init_start,
            router,
            assets_path,
            sessions: None,
            addr: addr.to_string(),
            workers: 1,
            shutdown_timeout: Duration::from_secs(30),
        })
    }

    #[cfg(not(target_os = "linux"))]
    fn bind_listener(addr: &str) -> io::Result<TcpListener> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        Ok(listener)
    }

    #[cfg(target_os = "linux")]
    fn bind_reuseport_listener(addr: &str) -> std::io::Result<std::net::TcpListener> {
        use std::io;
        use std::net::{SocketAddr, TcpListener};
        use std::os::unix::io::FromRawFd;

        let sock_addr: SocketAddr = addr
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        unsafe {
            let (domain, family) = match sock_addr {
                SocketAddr::V4(_) => (libc::AF_INET, libc::AF_INET),
                SocketAddr::V6(_) => (libc::AF_INET6, libc::AF_INET6),
            };

            let fd = libc::socket(
                domain,
                libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                0,
            );
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }

            let on: libc::c_int = 1;
            let opt_len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;

            if libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_REUSEADDR,
                &on as *const _ as *const libc::c_void,
                opt_len,
            ) < 0
                || libc::setsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_REUSEPORT,
                    &on as *const _ as *const libc::c_void,
                    opt_len,
                ) < 0
            {
                let err = io::Error::last_os_error();
                libc::close(fd);
                return Err(err);
            }

            // Keep local sockaddr structures alive in scope during bind()
            let bind_res = match sock_addr {
                SocketAddr::V4(a) => {
                    let sin = libc::sockaddr_in {
                        sin_family: family as libc::sa_family_t,
                        sin_port: a.port().to_be(),
                        sin_addr: libc::in_addr {
                            s_addr: u32::from_ne_bytes(a.ip().octets()),
                        },
                        sin_zero: [0; 8],
                    };
                    libc::bind(
                        fd,
                        &sin as *const _ as *const libc::sockaddr,
                        std::mem::size_of_val(&sin) as libc::socklen_t,
                    )
                }
                SocketAddr::V6(a) => {
                    let sin6 = libc::sockaddr_in6 {
                        sin6_family: family as libc::sa_family_t,
                        sin6_port: a.port().to_be(),
                        sin6_flowinfo: a.flowinfo(),
                        sin6_addr: libc::in6_addr {
                            s6_addr: a.ip().octets(),
                        },
                        sin6_scope_id: a.scope_id(),
                    };
                    libc::bind(
                        fd,
                        &sin6 as *const _ as *const libc::sockaddr,
                        std::mem::size_of_val(&sin6) as libc::socklen_t,
                    )
                }
            };

            if bind_res < 0 || libc::listen(fd, libc::SOMAXCONN) < 0 {
                let err = io::Error::last_os_error();
                libc::close(fd);
                return Err(err);
            }

            Ok(TcpListener::from_raw_fd(fd))
        }
    }

    fn bind_listeners(addr: &str, count: usize) -> io::Result<Vec<TcpListener>> {
        #[cfg(target_os = "linux")]
        {
            (0..count)
                .map(|_| Self::bind_reuseport_listener(addr))
                .collect()
        }

        #[cfg(not(target_os = "linux"))]
        {
            // Other Unix systems have no reliable REUSEPORT load balancing;
            // share one listening socket by duplicating its descriptor.
            // Non-blocking mode lives on the shared open file description,
            // so setting it once covers all duplicates.
            let primary = Self::bind_listener(addr)?;
            let mut listeners = Vec::with_capacity(count);
            listeners.push(primary);
            while listeners.len() < count {
                listeners.push(listeners[0].try_clone()?);
            }
            Ok(listeners)
        }
    }

    /// Run polling
    pub fn run(&mut self) -> io::Result<()> {
        let workers = self.workers.max(1);
        let shutdown_timeout = self.shutdown_timeout;

        // Install SIGINT/SIGTERM handling BEFORE binding so that once the
        // listening socket accepts connections, shutdown handlers are
        // guaranteed to be armed.
        let shutdown = Arc::new(Shutdown::install()?);

        let mut listeners = Self::bind_listeners(&self.addr, workers).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("failed to bind server to {}: {}", self.addr, e),
            )
        })?;

        let local_addr = listeners[0].local_addr()?;
        let ctx = Arc::new(WorkerCtx {
            router: Arc::clone(&self.router),
            assets_path: self.assets_path.clone(),
            sessions: self.sessions.clone(),
        });

        let main_listener = listeners.swap_remove(0);

        let mut handles = Vec::with_capacity(listeners.len());
        for (i, listener) in listeners.into_iter().enumerate() {
            let ctx = Arc::clone(&ctx);
            let shutdown = Arc::clone(&shutdown);

            let handle = thread::Builder::new()
                .name(format!("r-server-worker-{}", i + 1))
                .spawn(move || Self::event_loop(listener, &ctx, &shutdown, shutdown_timeout))
                .map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::OutOfMemory,
                        format!("failed to spawn worker thread {}: {}", i + 1, e),
                    )
                })?;
            handles.push(handle);
        }

        info!(
            "HTTP server started on http://{} with {} worker(s) in {}µs",
            local_addr,
            workers,
            self.init_start.elapsed().as_micros()
        );

        // Run the primary worker on the calling thread.
        let result = Self::event_loop(main_listener, &ctx, &shutdown, shutdown_timeout);

        // Signal auxiliary workers to stop and wait for them.
        shutdown.trigger();
        for handle in handles {
            let _ = handle.join();
        }

        // Stop repeating background tasks and restore default signal
        // dispositions (via Drop).
        task::cancel_all_and_join();
        drop(shutdown);

        result
    }

    pub fn route(
        &mut self,
        method: crate::router::Method,
        path: &str,
        handler: crate::router::HandlerFn,
    ) -> &mut Self {
        Arc::get_mut(&mut self.router)
            .expect("Cannot add route: Router is already shared across threads")
            .add_route(method, path, handler);
        self
    }

    pub fn use_middleware(&mut self, middleware: fn(&Request, &mut Response, Next)) -> &mut Self {
        Arc::get_mut(&mut self.router)
            .expect("Cannot add middleware: Router is already shared across threads")
            .use_middleware(middleware);
        self
    }

    fn would_block(err: &io::Error) -> bool {
        matches!(
            err.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
        )
    }

    fn handle_write(conn: &mut Connection<TcpStream>) -> io::Result<WriteState> {
        loop {
            if conn.write_buf.is_empty() {
                trace!("Write buffer empty; state: Done");
                return Ok(WriteState::Done);
            }

            // Dereference the Arc (`&*`) to call Write::write on `&TcpStream`
            match (&*conn.metadata.stream).write(&conn.write_buf) {
                Ok(0) => {
                    // Ok(0) usually means the connection was closed by the remote peer
                    debug!("Socket closed by peer (EOF on write); state: Close");
                    return Ok(WriteState::Close);
                }
                Ok(n) => {
                    // Use debug for successful progress to avoid flooding logs in production,
                    // but allow visibility during development.
                    debug!(
                        "Wrote {} bytes; remaining in buffer: {}",
                        n,
                        conn.write_buf.len() - n
                    );
                    conn.write_buf.drain(0..n);
                }
                Err(ref err) if Self::would_block(err) => {
                    // Would block is an expected part of non-blocking I/O, keep it at trace level
                    trace!("Write would block; state: Continue");
                    return Ok(WriteState::Continue);
                }
                Err(err) => {
                    // Actual errors (connection reset, etc.) are critical
                    error!("Failed to write to socket: {}", err);
                    return Err(err);
                }
            }
        }
    }

    fn handle_read(ctx: &WorkerCtx, conn: &mut Connection<TcpStream>) -> io::Result<bool> {
        let mut buf = [0; 1024];

        loop {
            // Call .read(&mut buf) on the underlying stream
            match (&*conn.metadata.stream).read(&mut buf) {
                Ok(0) => {
                    // A read of 0 bytes signifies the remote peer closed the connection.
                    debug!("Connection closed by peer (EOF); state: Terminating");
                    return Ok(false);
                }
                Ok(n) => {
                    debug!("Read {} bytes from socket", n);
                    conn.read_buf.extend_from_slice(&buf[..n]);
                }
                Err(ref err) if Self::would_block(err) => {
                    debug!("Read would block; returning control to event loop");
                    break;
                }
                Err(err) => {
                    error!("Socket read error: {}", err);
                    return Err(err);
                }
            }
        }

        // Attempt to parse the request from the accumulated buffer.
        if let Some(mut request) = Request::parse(&conn.read_buf) {
            trace!(
                "Request parsed successfully: {} {}",
                request.method, request.path
            );

            let metadata: Arc<dyn Metadata> = Arc::new(conn.metadata.clone());

            // Pass cloned Arc<dyn Metadata> instead of borrowed reference &conn.metadata
            let mut response = Response::new(metadata, Status::Ok, b"", ContentType::TEXT);

            // Resolve the browser session (when enabled) before routing so
            // handlers can read and mutate it; remember freshly minted ones
            // so the cookie can be announced on the response below.
            let mut attached_session = None;
            if let Some(store) = &ctx.sessions {
                let sid = request.header("cookie").and_then(sid_from_cookie);
                let (session, is_new) = store.get_or_create(sid);
                attached_session = Some((store.clone(), session.clone(), is_new));
                request.session = Some(session);
            }

            if let Some(handler_fn) = ctx.router.route(&mut request) {
                // Execute route handler inside middleware chain
                ctx.router.handle(&request, &mut response, handler_fn);
            } else {
                // Execute route for static resoueces
                ctx.router.static_handle(
                    &request,
                    &mut response,
                    Self::static_handler,
                    &ctx.assets_path, // Pass the static directory path
                );
            }

            // Announce or expire the session cookie after dispatch, since a
            // handler may have created or destroyed the session mid-request.
            if let Some((store, session, is_new)) = attached_session {
                if session.is_destroyed() {
                    store.destroy(&session.id());
                    response.header("Set-Cookie", cleared_session_cookie());
                } else if is_new {
                    let sid = session.id();
                    response.header("Set-Cookie", session_set_cookie(&sid, store.ttl()));
                }
            }

            // Prevent sending an empty body
            if response.body.is_empty() {
                return Ok(true);
            }

            // Prepare response for writing and clear buffer
            conn.write_buf = response.build();
            conn.read_buf.clear();
            trace!(
                "Response prepared; write_buf size: {} bytes",
                conn.write_buf.len()
            );
        } else {
            trace!("Buffer contains partial request; waiting for more data...");
        }

        Ok(true)
    }

    fn static_handler(req: &Request, res: &mut Response, path: &Path) {
        if let Some((content, content_type, etag, last_modified)) = get_file_info(&req.path, path) {
            res.body(content);
            res.content_type(content_type);
            res.header("Cache-control", "public, max-age=3600");

            if !etag.is_empty() {
                res.header("ETag", etag);
            }

            if !last_modified.is_empty() {
                res.header("Last-Modified", last_modified);
            }
        } else {
            res.body("Not Found");
            res.status(Status::NotFound);
        }
    }

    /// Single-worker polling loop. Each worker owns its listener and its
    /// connections exclusively; no synchronization is needed on the hot path.
    ///
    /// When a graceful shutdown is triggered the worker stops accepting,
    /// closes idle keep-alive connections immediately and keeps servicing
    /// only connections with a pending response until they are flushed or
    /// `drain_timeout` elapses.
    fn event_loop(
        listener: TcpListener,
        ctx: &WorkerCtx,
        shutdown: &Shutdown,
        drain_timeout: Duration,
    ) -> io::Result<()> {
        listener.set_nonblocking(true)?;

        // Poll set layout: [wake-pipe, listener (while accepting), clients...]
        const WAKE_INDEX: usize = 0;
        const LISTENER_INDEX: usize = 1;

        let mut poll_fds: Vec<PollFd> = vec![
            PollFd {
                fd: shutdown.wake_fd(),
                events: POLLIN,
                revents: 0,
            },
            PollFd {
                fd: listener.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            },
        ];

        // Taken (closed) as soon as draining starts.
        let mut listener = Some(listener);
        let mut connections: HashMap<i32, Connection<TcpStream>> = HashMap::new();
        let mut indices_to_remove = Vec::new();
        let mut drain_deadline: Option<Instant> = None;

        loop {
            let draining = drain_deadline.is_some();

            if !draining && shutdown.is_triggered() {
                info!(
                    "Graceful shutdown started; closing listener and draining {} connection(s)",
                    connections.len()
                );
                drain_deadline = Some(Instant::now() + drain_timeout);

                // Close the listening socket: new connections are refused
                // and the port is released while existing ones finish.
                drop(listener.take());
                poll_fds.remove(LISTENER_INDEX);

                // Idle keep-alive connections have nothing in flight; close
                // them right away and keep only responses waiting to flush.
                connections.retain(|_, conn| !conn.write_buf.is_empty());
                let wake_fd = shutdown.wake_fd();
                poll_fds.retain(|pfd| pfd.fd == wake_fd || connections.contains_key(&pfd.fd));

                debug!("Draining {} connection(s)", connections.len());
                continue;
            }

            if let Some(deadline) = drain_deadline {
                if connections.is_empty() {
                    debug!("All connections drained");
                    return Ok(());
                }

                if Instant::now() >= deadline {
                    debug!(
                        "Drain timeout exceeded; force-closing {} connection(s)",
                        connections.len()
                    );
                    // Dropping the map closes every remaining socket.
                    return Ok(());
                }
            }

            for pfd in poll_fds.iter_mut() {
                pfd.revents = 0;
            }

            let timeout = Shutdown::poll_timeout(drain_deadline, Duration::from_secs(POLL_TIMEOUT));
            let nfds = unsafe {
                libc::poll(
                    poll_fds.as_mut_ptr() as *mut libc::pollfd,
                    poll_fds.len() as libc::nfds_t,
                    timeout,
                )
            };

            if nfds < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    // Interrupted is a normal part of many system calls; skip silently or trace
                    trace!("Poll interrupted by signal");
                    continue;
                }
                // A real error in poll is critical
                error!("Fatal error during poll: {}", err);
                return Err(err);
            }

            if nfds == 0 {
                continue;
            }

            // Wake pipe: consume pending bytes so readiness clears.
            if poll_fds[WAKE_INDEX].revents & POLLIN != 0 {
                shutdown.drain_wake_pipe();
            }

            // Handle listener (index 1) while still accepting.
            if let Some(listener_ref) = listener.as_ref()
                && poll_fds[LISTENER_INDEX].revents & POLLIN != 0
            {
                loop {
                    match listener_ref.accept() {
                        Ok((stream, addr)) => {
                            let fd = stream.as_raw_fd();
                            let conn = Connection::new(stream)?;
                            poll_fds.push(PollFd {
                                fd,
                                events: POLLIN,
                                revents: 0,
                            });
                            connections.insert(fd, conn);
                            // is appropriate for a new connection event
                            debug!("New connection accepted from {} (FD: {})", addr, fd);
                        }
                        Err(ref err) if Self::would_block(err) => break,
                        Err(err) => {
                            // error! replaces the eprintln! to provide context
                            error!("Accept error on listener: {}", err);
                            break;
                        }
                    }
                }
            }

            indices_to_remove.clear();

            let listener_active = listener.is_some();
            for (i, item) in poll_fds.iter_mut().enumerate() {
                if i == WAKE_INDEX || (listener_active && i == LISTENER_INDEX) {
                    continue;
                }

                if item.revents == 0 {
                    continue;
                }

                let revents = item.revents;
                let fd = item.fd;

                // Check for socket errors or hang-ups (connection closed by peer)
                if revents & (POLLERR | POLLHUP) != 0 {
                    debug!("Socket error or hangup on FD: {}", fd);
                    indices_to_remove.push(i);
                    continue;
                }

                if revents & POLLOUT != 0 {
                    if let Some(conn) = connections.get_mut(&fd) {
                        match Self::handle_write(conn) {
                            Ok(WriteState::Done) => {
                                if draining {
                                    // Response fully flushed during shutdown;
                                    // this connection has nothing left to do.
                                    trace!("FD {}: flushed during shutdown; closing", fd);
                                    indices_to_remove.push(i);
                                } else {
                                    item.events = POLLIN;
                                }
                            }
                            Ok(WriteState::Continue) => {
                                // Trace is better here: it's high-frequency progress data
                                trace!("FD {}: still writing...", fd);
                            }
                            Ok(WriteState::Close) => {
                                debug!("FD {}: closing connection after write", fd);
                                indices_to_remove.push(i);
                            }
                            Err(err) => {
                                error!("FD {}: Write error: {}", fd, err);
                                indices_to_remove.push(i);
                            }
                        }
                    }
                } else if revents & POLLIN != 0 && !draining {
                    // New inbound requests are ignored during a drain.
                    if let Some(conn) = connections.get_mut(&fd) {
                        match Self::handle_read(ctx, conn) {
                            Ok(true) => {
                                if !conn.write_buf.is_empty() {
                                    item.events = POLLOUT;
                                }
                            }
                            Ok(false) => {
                                // Client closed connection gracefully (EOF)
                                debug!("FD {}: Connection closed by client", fd);
                                indices_to_remove.push(i);
                            }
                            Err(err) => {
                                error!("FD {}: Read error: {}", fd, err);
                                indices_to_remove.push(i);
                            }
                        }
                    }
                }
            }

            // Cleanup phase
            for i in indices_to_remove.iter().rev() {
                let fd = poll_fds[*i].fd;
                connections.remove(&fd);
                poll_fds.remove(*i);
                trace!("FD {}: Removed from event loop", fd);
            }
        }
    }
}

#[cfg(test)]
mod tests;
