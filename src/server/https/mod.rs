use crate::server::connection::{ConnectionMetadata, ConnectionStreamClone};
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
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::request::Request;
use crate::response::{ContentType, Response, Status};
use crate::router::Router;

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

struct Connection {
    metadata: ConnectionMetadata<Option<SharedTlsState>>,
    read_buf: Vec<u8>,
    write_buf: Vec<u8>,
}

impl Connection {
    fn new(tls: TlsState) -> Self {
        Self {
            metadata: ConnectionMetadata {
                stream: Some(Arc::new(Mutex::new(Some(tls)))),
            },
            read_buf: Vec::with_capacity(1024),
            write_buf: Vec::new(),
        }
    }

    fn fd(&self) -> i32 {
        let shared = self
            .metadata
            .stream
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

impl ConnectionStreamClone for Option<Arc<Mutex<Option<TlsState>>>> {
    fn clone_stream(&self) -> io::Result<Self> {
        Ok(self.clone())
    }
}

enum WriteState {
    Continue,
    Done,
    Close,
}

pub struct Server {
    init_start: Instant,
    listener: TcpListener,
    router: Arc<Router<Option<SharedTlsState>>>,
    assets_path: PathBuf,
    acceptor: Arc<SslAcceptor>,
}

impl Server {
    pub fn new(addr: &str) -> io::Result<Self> {
        Self::new_with_assets(addr, PathBuf::from("./assets"))
    }

    pub fn assets_path(&mut self, path: &str) -> &mut Self {
        self.assets_path = PathBuf::from(path);
        self
    }

    pub fn route(
        &mut self,
        method: crate::router::Method,
        path: &str,
        handler: crate::router::HandlerFn<Option<SharedTlsState>>,
    ) -> &mut Self {
        if let Some(router) = Arc::get_mut(&mut self.router) {
            trace!("Successfully added route: {} {}", method.index(), path);
            router.add_route(method, path, handler);
        }

        self
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.run_loop()
    }

    fn new_with_assets(addr: &str, assets_path: PathBuf) -> io::Result<Self> {
        let init_start = Instant::now();

        let router = Arc::new(Router::new());

        let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();

        match builder.set_private_key_file("key.pem", SslFiletype::PEM) {
            Ok(_) => {}
            Err(_) => {
                error!("ERROR: Failed to load 'key.pem'");
                panic!("Server initialization failed: 'key.pem' not found.");
            }
        }

        match builder.set_certificate_chain_file("cert.pem") {
            Ok(_) => {}
            Err(_) => {
                error!("ERROR: Failed to load 'cert.pem'");
                panic!("Server initialization failed: 'cert.pem' not found.");
            }
        }

        Ok(Server {
            init_start,
            listener: TcpListener::bind(addr.parse::<std::net::SocketAddr>().unwrap())?,
            router,
            assets_path,
            acceptor: Arc::new(builder.build()),
        })
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

            let shared = conn.metadata.stream.as_ref().ok_or_else(|| {
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

    fn handle_read(
        conn: &mut Connection,
        router: &Router<Option<SharedTlsState>>,
        assets_path: &Path,
    ) -> io::Result<bool> {
        let mut buf = [0u8; 1024];

        loop {
            let read_result = {
                let shared = conn.metadata.stream.as_ref().ok_or_else(|| {
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

            let mut response = Response::new(&conn.metadata, Status::Ok, b"", ContentType::TEXT);

            if let Some(handler_fn) = router.route(&mut request) {
                handler_fn(&request, &mut response);
            } else if let Some((content, content_type, etag, last_modified)) =
                get_file_info(request.path, assets_path)
            {
                response.body(content);
                response.content_type(content_type);
                response.header("Cache-control", "public, max-age=3600");

                if !etag.is_empty() {
                    response.header("ETag", etag);
                }

                if !last_modified.is_empty() {
                    response.header("Last-Modified", last_modified);
                }
            } else {
                response.body("Not Found");
                response.status(Status::NotFound);
            }

            // Prevent sending an empty body
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

    fn continue_handshake(conn: &mut Connection) -> io::Result<bool> {
        let shared = conn.metadata.stream.as_ref().ok_or_else(|| {
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

            TlsState::Handshaking(mid) => {
                match mid.handshake() {
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

                        // Don't put a failed TLS state back.
                        *state = None;

                        Err(io::Error::other(format!("{:?}", e)))
                    }
                }
            }
        }
    }

    fn run_loop(&mut self) -> io::Result<()> {
        self.listener.set_nonblocking(true)?;

        let mut poll_fds: Vec<PollFd> = vec![PollFd {
            fd: self.listener.as_raw_fd(),
            events: POLLIN,
            revents: 0,
        }];

        let mut connections: HashMap<i32, Connection> = HashMap::new();

        let startup_us = self.init_start.elapsed().as_micros();

        info!(
            "HTTPS server started on https://{} in {}µs",
            self.listener.local_addr()?,
            startup_us
        );

        let mut indices_to_remove = Vec::new();

        loop {
            for pfd in poll_fds.iter_mut() {
                pfd.revents = 0;
            }

            let nfds = unsafe {
                libc::poll(
                    poll_fds.as_mut_ptr() as *mut libc::pollfd,
                    poll_fds.len() as libc::nfds_t,
                    2000,
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

            // Accept HTTPS clients.
            if poll_fds[0].revents & POLLIN != 0 {
                loop {
                    match self.listener.accept() {
                        Ok((stream, addr)) => {
                            stream.set_nonblocking(true)?;

                            let tls_state = match self.acceptor.accept(stream) {
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
                        let shared = match conn.metadata.stream.as_ref() {
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
                                    "Connection FD {} closed by remote peer \
                                     (WriteState::Close)",
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
                        match Self::handle_read(conn, &self.router, &self.assets_path) {
                            Ok(true) => {
                                if !conn.write_buf.is_empty() {
                                    item.events = POLLOUT;
                                }
                            }

                            Ok(false) => {
                                debug!(
                                    "Connection FD {} closed by remote peer \
                                     (Read finished)",
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
