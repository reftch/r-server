use crate::{debug, error, info, trace, warn};
use libc::{POLLERR, POLLHUP, POLLIN, POLLOUT};
use openssl::ssl::{
    HandshakeError, MidHandshakeSslStream, SslAcceptor, SslFiletype, SslMethod, SslStream,
};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::request::Request;
use crate::response::{ContentType, Response, Status};
use crate::router::Router;

use std::sync::Arc;

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

enum TlsState {
    Handshaking(MidHandshakeSslStream<TcpStream>),
    Connected(SslStream<TcpStream>),
}

struct Connection {
    tls: Option<TlsState>,
    read_buf: Vec<u8>,
    write_buf: Vec<u8>,
}

impl Connection {
    fn new(tls: TlsState) -> Self {
        Self {
            tls: Some(tls),
            read_buf: Vec::with_capacity(1024),
            write_buf: Vec::new(),
        }
    }

    fn fd(&self) -> i32 {
        match self.tls.as_ref().unwrap() {
            TlsState::Connected(s) => s.get_ref().as_raw_fd(),
            TlsState::Handshaking(s) => s.get_ref().as_raw_fd(),
        }
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
    router: Arc<Router>,
    assets_path: PathBuf,
    acceptor: Arc<SslAcceptor>,
}

impl Server {
    pub fn new(addr: &str) -> io::Result<Self> {
        Self::new_with_assets(addr, PathBuf::from("./assets"))
    }

    fn new_with_assets(addr: &str, assets_path: PathBuf) -> io::Result<Self> {
        let init_start = Instant::now();

        let router = Arc::new(Router::new());
        let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();

        // Try to load the private key. If it fails, log and crash the app.
        match builder.set_private_key_file("key.pem", SslFiletype::PEM) {
            Ok(_) => true,
            Err(_) => {
                error!("ERROR: Failed to load 'key.pem'");
                panic!("Server initialization failed: 'key.pem' not found.");
            }
        };

        // Try to load the certificate. If it fails, log and crash the app.
        match builder.set_certificate_chain_file("cert.pem") {
            Ok(_) => true,
            Err(_) => {
                error!("ERROR: Failed to load 'cert.pem'");
                panic!("Server initialization failed: 'cert.pem' not found.");
            }
        };

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
                // Log that the buffer is clear and we are finished
                trace!("Write buffer is empty. Finishing write state.");
                return Ok(WriteState::Done);
            }

            match conn.tls.as_mut().unwrap() {
                TlsState::Handshaking(_) => {
                    // TLS is not ready yet
                    return Ok(WriteState::Continue);
                }

                TlsState::Connected(stream) => match stream.write(&conn.write_buf) {
                    Ok(0) => {
                        // Log that the remote side closed the connection
                        debug!("Socket closed by peer (EOF on write); state: Close");
                        return Ok(WriteState::Close);
                    }
                    Ok(n) => {
                        // Log progress of data being sent
                        debug!(
                            "Wrote {} bytes; remaining in buffer: {}",
                            n,
                            conn.write_buf.len() - n
                        );
                        conn.write_buf.drain(0..n);
                    }
                    Err(ref err) if Self::would_block(err) => {
                        trace!("Write would block; state: Continue");
                        return Ok(WriteState::Continue);
                    }
                    Err(err) => {
                        // Log the actual I/O error before returning it
                        error!("Failed to write to socket: {}", err);
                        return Err(err);
                    }
                },
            }
        }
    }

    fn handle_read(conn: &mut Connection, router: &Router, assets_path: &Path) -> io::Result<bool> {
        let mut buf = [0u8; 1024];

        loop {
            let stream = match conn.tls.as_mut().unwrap() {
                TlsState::Connected(stream) => stream,

                // TLS handshake is not complete yet
                TlsState::Handshaking(_) => {
                    return Ok(true);
                }
            };

            match stream.read(&mut buf) {
                Ok(0) => {
                    // A read of 0 bytes usually signifies the peer has closed the connection.
                    debug!("Connection closed by peer (EOF); state: Terminating");
                    return Ok(false);
                }
                Ok(n) => {
                    // Log successful reads at debug level to track data influx without spamming logs.
                    debug!("Read {} bytes from socket", n);
                    conn.read_buf.extend_from_slice(&buf[..n]);
                }
                Err(ref err) if Self::would_block(err) => {
                    // Expected behavior in non-blocking I/O; move to processing phase.
                    debug!("Read would block; returning control to event loop");
                    break;
                }
                Err(err) => {
                    // Actual I/O error (e.g., ConnectionReset).
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

            let response = if let Some(resp) = router.route(&mut request) {
                trace!("Route matched for path: {}", request.path);
                resp
            } else if let Some(resp) = Self::handle_static(request.path, assets_path) {
                trace!("Static asset found: {}", request.path);
                resp
            } else {
                warn!("Route not found: {}", request.path);
                Response::new(Status::NotFound, "Not Found", ContentType::TEXT)
            };

            conn.write_buf = response.build();
            conn.read_buf.clear();
            trace!(
                "Response prepared; write_buf size: {} bytes",
                conn.write_buf.len()
            );
        } else {
            // We don't log anything here if the buffer is just incomplete,
            // as that would be too noisy (the loop might trigger many times for a partial request).
            trace!("Buffer contains partial request; waiting for more data...");
        }

        Ok(true)
    }

    fn handle_static(path: &str, assets_path: &Path) -> Option<Response> {
        let requested_path = Path::new(path);

        // Prevent directory traversal attacks (e.g., "/../etc/passwd")
        if requested_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return None;
        }

        let mut full_path =
            assets_path.join(requested_path.strip_prefix("/").unwrap_or(requested_path));

        if full_path.is_dir() {
            // If a directory is requested, default to index.html as per standard web server behavior
            full_path.push("index.html");
        }

        if !full_path.is_file() {
            return None;
        }

        // Attempt to read the file from the filesystem
        match fs::read(&full_path) {
            Ok(content) => {
                let content_type = Self::get_content_type(&full_path);
                trace!(
                    "Successfully read static file: {} ({} bytes)",
                    full_path.display(),
                    content.len()
                );

                Some(Response {
                    status: Status::Ok,
                    body: content,
                    content_type,
                    headers: HashMap::new(),
                })
            }
            Err(err) => {
                // Log as error because a file that 'should' exist but can't be read
                // is a filesystem issue (permissions, locked files, etc.)
                error!("Failed to read static file {:?}: {}", full_path, err);
                None
            }
        }
    }

    fn get_content_type(path: &Path) -> ContentType {
        match path.extension().and_then(|s| s.to_str()) {
            Some("html") => ContentType::HTML,
            Some("css") => ContentType::CSS,
            Some("js") => ContentType::JAVASCRIPT,
            Some("jpg") | Some("jpeg") => ContentType::JPEG,
            Some("png") => ContentType::PNG,
            Some("xml") => ContentType::XML,
            Some("json") => ContentType::JSON,
            Some("txt") => ContentType::TEXT,
            Some("gif") => ContentType::GIF,
            Some("svg") => ContentType::SVG,
            Some("pdf") => ContentType::PDF,
            Some("mp3") => ContentType::MP3,
            Some("mp4") => ContentType::MP4,
            Some("webm") => ContentType::WEBM,
            Some("woff2") => ContentType::WOFF2,
            Some("ttf") => ContentType::TTF,
            _ => ContentType::UNKNOWN,
        }
    }

    pub fn assets_path(&mut self, path: &str) {
        self.assets_path = PathBuf::from(path);
    }

    pub fn route(
        &mut self,
        method: crate::router::Method,
        path: &str,
        handler: crate::router::HandlerFn,
    ) -> &mut Self {
        if let Some(router) = std::sync::Arc::get_mut(&mut self.router) {
            trace!("Successfully added route: {} {}", method.index(), path);
            router.add_route(method, path, handler);
        }
        self
    }

    pub fn run(&mut self) -> io::Result<()> {
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

            // Accept HTTPS clients
            if poll_fds[0].revents & POLLIN != 0 {
                loop {
                    match self.listener.accept() {
                        Ok((stream, _addr)) => {
                            stream.set_nonblocking(true)?;

                            let tls_state = match self.acceptor.accept(stream) {
                                Ok(ssl) => TlsState::Connected(ssl),
                                Err(HandshakeError::WouldBlock(mid)) => TlsState::Handshaking(mid),
                                Err(e) => {
                                    // Log handshake failures (like bad protocols or missing certs)
                                    warn!("TLS handshake initialization failed: {:?}", e);
                                    continue;
                                }
                            };

                            let conn = Connection::new(tls_state);
                            let fd = conn.fd();

                            // Log new connection attempt
                            debug!("New connection accepted: FD {} from {}", fd, _addr);

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
                            error!("Accept error: {}", err); // Replaced eprintln with error!
                            break;
                        }
                    }
                }
            }

            indices_to_remove.clear();

            // Client connections
            for (i, item) in poll_fds.iter_mut().enumerate().skip(1) {
                if item.revents == 0 {
                    continue;
                }

                let fd = item.fd;
                let events = item.revents;

                // Handle unexpected connection drops (Client disconnected or error occurred at OS level)
                if events & (POLLERR | POLLHUP) != 0 {
                    debug!("Connection FD {} closed via poll event (ERR/HUP)", fd);
                    indices_to_remove.push(i);
                    continue;
                }

                if let Some(conn) = connections.get_mut(&fd) {
                    // Finish TLS handshake
                    if matches!(conn.tls.as_ref(), Some(TlsState::Handshaking(_))) {
                        match Self::continue_handshake(conn) {
                            Ok(true) => {
                                // Log when the handshake is successfully finished
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

                    // Write HTTPS response
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

                    // Read HTTPS request
                    if events & POLLIN != 0 {
                        match Self::handle_read(conn, &self.router, &self.assets_path) {
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

            // Remove closed connections
            for i in indices_to_remove.iter().rev() {
                let fd = poll_fds[*i].fd;
                connections.remove(&fd);
                poll_fds.remove(*i);
            }
        }
    }

    fn continue_handshake(conn: &mut Connection) -> io::Result<bool> {
        let state = conn.tls.take().unwrap();

        match state {
            TlsState::Connected(stream) => {
                conn.tls = Some(TlsState::Connected(stream));
                Ok(true)
            }

            TlsState::Handshaking(mid) => match mid.handshake() {
                Ok(stream) => {
                    // Log successful completion of the handshake
                    debug!("TLS handshake completed successfully.");
                    conn.tls = Some(TlsState::Connected(stream));
                    Ok(true)
                }
                Err(HandshakeError::WouldBlock(mid)) => {
                    // NOTE: We do NOT log WouldBlock here because it is a normal,
                    // high-frequency event in non-blocking I/O. Logging this would
                    // cause massive performance issues and log spam.
                    conn.tls = Some(TlsState::Handshaking(mid));
                    Ok(false)
                }
                Err(e) => {
                    // Log actual errors that prevent the handshake from completing
                    error!("TLS handshake failed: {:?}", e);
                    Err(io::Error::other(format!("{:?}", e)))
                }
            },
        }
    }
}

#[cfg(test)]
mod tests;
