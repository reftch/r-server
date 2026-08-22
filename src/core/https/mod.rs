use crate::core::connection::{ConnectionMetadata, ConnectionStreamClone};
use crate::core::metadata::Metadata;
use crate::utils::get_file_info;
use crate::{debug, error, info, trace, warn};
use libc::{POLLERR, POLLHUP, POLLIN, POLLOUT};
use openssl::ssl::{
    HandshakeError, MidHandshakeSslStream, SslAcceptor, SslFiletype, SslMethod, SslStream,
};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use crate::request::Request;
use crate::response::{ContentType, Response, Status};
use crate::router::{Next, Router};

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

pub enum TlsState {
    Handshaking(MidHandshakeSslStream<TcpStream>),
    Connected(SslStream<TcpStream>),
}

type SharedTlsState = Arc<Mutex<Option<TlsState>>>;

// 1. Local wrapper struct to bypass the orphan rule
#[derive(Clone)]
pub struct TlsStreamHandle(pub Option<SharedTlsState>);

// 2. Implement Write on the reference of our local wrapper struct
impl Write for &TlsStreamHandle {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let shared = self.0.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "TLS stream uninitialized")
        })?;

        let mut state = shared
            .lock()
            .map_err(|_| io::Error::other("TLS stream mutex poisoned"))?;

        let tls_state = state
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "TLS state empty"))?;

        match tls_state {
            TlsState::Connected(stream) => stream.write(buf),
            TlsState::Handshaking(_) => Err(io::ErrorKind::WouldBlock.into()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let shared = self.0.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "TLS stream uninitialized")
        })?;

        let mut state = shared
            .lock()
            .map_err(|_| io::Error::other("TLS stream mutex poisoned"))?;

        let tls_state = state
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "TLS state empty"))?;

        match tls_state {
            TlsState::Connected(stream) => stream.flush(),
            TlsState::Handshaking(_) => Ok(()),
        }
    }
}

// 3. Implement ConnectionStreamClone on the local wrapper struct
impl ConnectionStreamClone for TlsStreamHandle {
    fn clone_stream(&self) -> io::Result<Self> {
        Ok(self.clone())
    }
}

struct Connection {
    metadata: ConnectionMetadata<TlsStreamHandle>,
    read_buf: Vec<u8>,
    write_buf: Vec<u8>,
}

impl Connection {
    fn new(tls: TlsState) -> Self {
        Self {
            metadata: ConnectionMetadata {
                stream: Arc::new(TlsStreamHandle(Some(Arc::new(Mutex::new(Some(tls)))))),
            },
            read_buf: Vec::with_capacity(1024),
            write_buf: Vec::new(),
        }
    }

    fn fd(&self) -> i32 {
        let shared = self
            .metadata
            .stream
            .0
            .as_ref()
            .expect("Stream should be initialized");

        let state = shared.lock().expect("TLS mutex poisoned");

        let tls_state = state.as_ref().expect("TLS state should be initialized");

        match tls_state {
            TlsState::Connected(stream) => stream.get_ref().as_raw_fd(),
            TlsState::Handshaking(stream) => stream.get_ref().as_raw_fd(),
        }
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
    acceptor: Arc<SslAcceptor>,
}

pub struct Server {
    init_start: Instant,
    router: Arc<Router>,
    assets_path: PathBuf,
    acceptor: Arc<SslAcceptor>,
    addr: String,
    workers: usize,
}

impl Server {
    pub fn new() -> io::Result<Self> {
        let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

        let port: u16 = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(8443);

        let addr = format!("{}:{}", host, port);

        Self::new_with_assets(&addr, PathBuf::from("./assets"))
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

    fn new_with_assets(addr: &str, assets_path: PathBuf) -> io::Result<Self> {
        let init_start = Instant::now();
        let router = Arc::new(Router::new());

        // Validate the address, but DON'T bind the socket here.
        addr.parse::<std::net::SocketAddr>()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls())
            .map_err(|e| io::Error::other(format!("failed to create SSL acceptor: {}", e)))?;

        builder
            .set_private_key_file("key.pem", SslFiletype::PEM)
            .map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("failed to load 'key.pem': {}", e),
                )
            })?;

        builder
            .set_certificate_chain_file("cert.pem")
            .map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("failed to load 'cert.pem': {}", e),
                )
            })?;
        let acceptor = builder.build();

        Ok(Server {
            init_start,
            router,
            assets_path,
            acceptor: Arc::new(acceptor),
            addr: addr.to_string(),
            workers: 1,
        })
    }

    fn bind_listener(addr: &str) -> io::Result<TcpListener> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        Ok(listener)
    }

    #[cfg(target_os = "linux")]
    fn bind_reuseport_listener(addr: &str) -> io::Result<TcpListener> {
        use std::net::SocketAddr;
        use std::os::unix::io::FromRawFd;

        let sock_addr: SocketAddr = addr
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        let (domain, addr_ptr, addr_len) = match sock_addr {
            SocketAddr::V4(ref a) => (
                libc::AF_INET,
                a as *const libc::sockaddr_in as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
            ),
            SocketAddr::V6(ref a) => (
                libc::AF_INET6,
                a as *const libc::sockaddr_in6 as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t,
            ),
        };

        unsafe {
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
                &on as *const libc::c_int as *const libc::c_void,
                opt_len,
            ) < 0
                || libc::setsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_REUSEPORT,
                    &on as *const libc::c_int as *const libc::c_void,
                    opt_len,
                ) < 0
            {
                let err = io::Error::last_os_error();
                libc::close(fd);
                return Err(err);
            }

            if libc::bind(fd, addr_ptr, addr_len) < 0 || libc::listen(fd, libc::SOMAXCONN) < 0 {
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

    pub fn run(&mut self) -> io::Result<()> {
        // The socket is created ONLY when the server actually starts.
        let workers = self.workers.max(1);
        let mut listeners = Self::bind_listeners(&self.addr, workers).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("failed to bind HTTPS server to {}: {}", self.addr, e),
            )
        })?;

        let local_addr = listeners[0].local_addr()?;
        let ctx = Arc::new(WorkerCtx {
            router: Arc::clone(&self.router),
            assets_path: self.assets_path.clone(),
            acceptor: Arc::clone(&self.acceptor),
        });

        let shutdown = Arc::new(AtomicBool::new(false));
        let main_listener = listeners.swap_remove(0);

        let mut handles = Vec::with_capacity(listeners.len());
        for (i, listener) in listeners.into_iter().enumerate() {
            let ctx = Arc::clone(&ctx);
            let shutdown = Arc::clone(&shutdown);

            let handle = thread::Builder::new()
                .name(format!("r-server-https-worker-{}", i + 1))
                .spawn(move || Self::event_loop(&listener, &ctx, &shutdown))
                .map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::OutOfMemory,
                        format!("failed to spawn worker thread {}: {}", i + 1, e),
                    )
                })?;
            handles.push(handle);
        }

        info!(
            "HTTPS server started on https://{} with {} worker(s) in {}µs",
            local_addr,
            workers,
            self.init_start.elapsed().as_micros()
        );

        // Run the primary worker on the calling thread.
        let result = Self::event_loop(&main_listener, &ctx, &shutdown);

        // Signal auxiliary workers to stop and wait for them.
        shutdown.store(true, Ordering::Relaxed);
        for handle in handles {
            let _ = handle.join();
        }

        result
    }

    pub fn route(
        &mut self,
        method: crate::router::Method,
        path: &str,
        handler: crate::router::HandlerFn,
    ) -> &mut Self {
        if let Some(router) = Arc::get_mut(&mut self.router) {
            trace!("Successfully added route: {} {}", method.index(), path);
            router.add_route(method, path, handler);
        }

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

    fn handle_write(conn: &mut Connection) -> io::Result<WriteState> {
        loop {
            if conn.write_buf.is_empty() {
                trace!("Write buffer is empty. Finishing write state.");
                return Ok(WriteState::Done);
            }

            let shared = conn.metadata.stream.0.as_ref().ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotConnected, "TLS stream is not initialized")
            })?;

            let result = {
                let mut state = shared
                    .lock()
                    .map_err(|_| io::Error::other("TLS stream mutex is poisoned"))?;

                let tls_state = state.as_mut().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotConnected, "TLS state is not initialized")
                })?;

                match tls_state {
                    TlsState::Handshaking(_) => {
                        return Ok(WriteState::Continue);
                    }

                    TlsState::Connected(stream) => stream.write(&conn.write_buf),
                }
            };

            match result {
                Ok(0) => {
                    debug!("Socket closed by peer (EOF on write); state: Close");
                    return Ok(WriteState::Close);
                }

                Ok(n) => {
                    debug!(
                        "Wrote {} bytes; remaining in buffer: {}",
                        n,
                        conn.write_buf.len() - n
                    );

                    conn.write_buf.drain(..n);
                }

                Err(ref err) if Self::would_block(err) => {
                    trace!("Write would block; state: Continue");
                    return Ok(WriteState::Continue);
                }

                Err(err) => {
                    error!("Failed to write to socket: {}", err);
                    return Err(err);
                }
            }
        }
    }

    fn handle_read(ctx: &WorkerCtx, conn: &mut Connection) -> io::Result<bool> {
        let mut buf = [0u8; 1024];

        loop {
            let read_result = {
                let shared = conn.metadata.stream.0.as_ref().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotConnected, "TLS stream is not initialized")
                })?;

                let mut state = shared
                    .lock()
                    .map_err(|_| io::Error::other("TLS stream mutex is poisoned"))?;

                let tls_state = state.as_mut().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotConnected, "TLS state is not initialized")
                })?;

                match tls_state {
                    TlsState::Connected(stream) => stream.read(&mut buf),

                    TlsState::Handshaking(_) => {
                        return Ok(true);
                    }
                }
            };

            match read_result {
                Ok(0) => {
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

        if let Some(mut request) = Request::parse(&conn.read_buf) {
            trace!(
                "Request parsed successfully: {} {}",
                request.method, request.path
            );

            let metadata: Arc<dyn Metadata> = Arc::new(conn.metadata.clone());
            let mut response = Response::new(metadata, Status::Ok, b"", ContentType::TEXT);

            if let Some(handler_fn) = ctx.router.route(&mut request) {
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

            if response.body.is_empty() {
                return Ok(true);
            }

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

    fn continue_handshake(conn: &mut Connection) -> io::Result<bool> {
        let shared = conn.metadata.stream.0.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "TLS stream is not initialized")
        })?;

        let mut state = shared
            .lock()
            .map_err(|_| io::Error::other("TLS stream mutex is poisoned"))?;

        let tls_state = state.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "TLS stream is not initialized")
        })?;

        match tls_state {
            TlsState::Connected(stream) => {
                *state = Some(TlsState::Connected(stream));
                Ok(true)
            }

            TlsState::Handshaking(mid) => match mid.handshake() {
                Ok(stream) => {
                    debug!("TLS handshake completed successfully.");
                    *state = Some(TlsState::Connected(stream));
                    Ok(true)
                }

                Err(HandshakeError::WouldBlock(mid)) => {
                    *state = Some(TlsState::Handshaking(mid));
                    Ok(false)
                }

                Err(e) => {
                    trace!("TLS handshake failed: {:?}", e);
                    *state = None;
                    Err(io::Error::other(format!("{:?}", e)))
                }
            },
        }
    }

    /// Single-worker polling loop. Each worker owns its listener and its
    /// connections exclusively; no synchronization is needed on the hot path.
    fn event_loop(listener: &TcpListener, ctx: &WorkerCtx, shutdown: &AtomicBool) -> io::Result<()> {
        listener.set_nonblocking(true)?;

        let mut poll_fds: Vec<PollFd> = vec![PollFd {
            fd: listener.as_raw_fd(),
            events: POLLIN,
            revents: 0,
        }];

        let mut connections: HashMap<i32, Connection> = HashMap::new();
        let mut indices_to_remove = Vec::new();

        loop {
            if shutdown.load(Ordering::Relaxed) {
                debug!("Worker shutting down");
                return Ok(());
            }

            for pfd in poll_fds.iter_mut() {
                pfd.revents = 0;
            }

            let nfds = unsafe {
                libc::poll(
                    poll_fds.as_mut_ptr() as *mut libc::pollfd,
                    poll_fds.len() as libc::nfds_t,
                    1_000, // Wake up periodically to check the shutdown flag
                )
            };

            if nfds < 0 {
                let err = io::Error::last_os_error();

                if err.kind() == io::ErrorKind::Interrupted {
                    trace!("Poll interrupted by signal");
                    continue;
                }

                error!("Fatal error during poll: {}", err);
                return Err(err);
            }

            if nfds == 0 {
                continue;
            }

            if poll_fds[0].revents & POLLIN != 0 {
                loop {
                    match listener.accept() {
                        Ok((stream, addr)) => {
                            stream.set_nonblocking(true)?;

                            let tls_state = match ctx.acceptor.accept(stream) {
                                Ok(ssl) => TlsState::Connected(ssl),
                                Err(HandshakeError::WouldBlock(mid)) => TlsState::Handshaking(mid),
                                Err(e) => {
                                    warn!("TLS handshake initialization failed: {:?}", e);
                                    continue;
                                }
                            };

                            let conn = Connection::new(tls_state);
                            let fd = conn.fd();

                            debug!("New connection accepted: FD {} from {}", fd, addr);

                            poll_fds.push(PollFd {
                                fd,
                                events: POLLIN | POLLOUT,
                                revents: 0,
                            });

                            connections.insert(fd, conn);
                        }

                        Err(ref err) if Self::would_block(err) => {
                            break;
                        }

                        Err(err) => {
                            error!("Accept error: {}", err);
                            break;
                        }
                    }
                }
            }

            indices_to_remove.clear();

            for (i, item) in poll_fds.iter_mut().enumerate().skip(1) {
                if item.revents == 0 {
                    continue;
                }

                let fd = item.fd;
                let events = item.revents;

                if events & (POLLERR | POLLHUP) != 0 {
                    debug!("Connection FD {} closed via poll event (ERR/HUP)", fd);

                    indices_to_remove.push(i);
                    continue;
                }

                if let Some(conn) = connections.get_mut(&fd) {
                    let handshaking = {
                        let shared = match conn.metadata.stream.0.as_ref() {
                            Some(shared) => shared,
                            None => {
                                indices_to_remove.push(i);
                                continue;
                            }
                        };

                        let state = match shared.lock() {
                            Ok(state) => state,
                            Err(_) => {
                                error!("TLS stream mutex poisoned");
                                indices_to_remove.push(i);
                                continue;
                            }
                        };

                        matches!(state.as_ref(), Some(TlsState::Handshaking(_)))
                    };

                    if handshaking {
                        match Self::continue_handshake(conn) {
                            Ok(true) => {
                                debug!("TLS Handshake completed for FD {}", fd);
                                item.events = POLLIN;
                            }

                            Ok(false) => {
                                item.events = POLLIN | POLLOUT;
                                continue;
                            }

                            Err(_) => {
                                debug!("TLS Handshake failed for FD {}", fd);
                                indices_to_remove.push(i);
                                continue;
                            }
                        }
                    }

                    if events & POLLOUT != 0 {
                        match Self::handle_write(conn) {
                            Ok(WriteState::Done) => {
                                item.events = POLLIN;
                            }

                            Ok(WriteState::Continue) => {
                                item.events = POLLOUT;
                            }

                            Ok(WriteState::Close) => {
                                debug!(
                                    "Connection FD {} closed by remote peer (WriteState::Close)",
                                    fd
                                );
                                indices_to_remove.push(i);
                            }

                            Err(e) => {
                                error!("Write error on FD {}: {}", fd, e);
                                indices_to_remove.push(i);
                            }
                        }
                    }

                    if events & POLLIN != 0 {
                        match Self::handle_read(ctx, conn) {
                            Ok(true) => {
                                if !conn.write_buf.is_empty() {
                                    item.events = POLLOUT;
                                }
                            }

                            Ok(false) => {
                                debug!(
                                    "Connection FD {} closed by remote peer (Read finished)",
                                    fd
                                );
                                indices_to_remove.push(i);
                            }

                            Err(e) => {
                                error!("Read error on FD {}: {}", fd, e);
                                indices_to_remove.push(i);
                            }
                        }
                    }
                }
            }

            for i in indices_to_remove.iter().rev() {
                let fd = poll_fds[*i].fd;

                connections.remove(&fd);
                poll_fds.remove(*i);
            }
        }
    }
}

#[cfg(test)]
mod tests;
