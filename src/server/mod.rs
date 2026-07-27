use libc::{POLLERR, POLLHUP, POLLIN, POLLOUT};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
// use std::os::fd::AsFd;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::request::Request;
use crate::response::{ContentType, Response, Status};
use crate::router::Router;
use crate::{debug, error, info, trace, warn};

use std::sync::Arc;

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}
#[derive(Debug)]
pub struct ConnectionMetadata<TcpStream> {
    pub stream: TcpStream,
}

struct Connection<T> {
    metadata: ConnectionMetadata<T>,
    read_buf: Vec<u8>,
    write_buf: Vec<u8>,
}

impl Connection<TcpStream> {
    fn new(socket: TcpStream) -> io::Result<Self> {
        socket.set_nonblocking(true)?;
        Ok(Self {
            metadata: ConnectionMetadata { stream: socket },
            read_buf: Vec::with_capacity(1024),
            write_buf: Vec::new(),
        })
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
}

impl Server {
    pub fn new(addr: &str) -> io::Result<Self> {
        Self::new_with_assets(addr, PathBuf::from("./assets"))
    }

    fn new_with_assets(addr: &str, assets_path: PathBuf) -> io::Result<Self> {
        let init_start = Instant::now();
        let router = Arc::new(Router::new());
        Ok(Server {
            init_start,
            listener: TcpListener::bind(addr.parse::<std::net::SocketAddr>().unwrap())?,
            router,
            assets_path,
        })
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

            match conn.metadata.stream.write(&conn.write_buf) {
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

    fn handle_read(
        conn: &mut Connection<TcpStream>,
        router: &Router,
        assets_path: &Path,
    ) -> io::Result<bool> {
        let mut buf = [0; 1024];
        loop {
            match conn.metadata.stream.read(&mut buf) {
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

        // Attempt to parse the request from the accumulated buffer.
        if let Some(mut request) = Request::parse(&conn.read_buf) {
            trace!(
                "Request parsed successfully: {} {}",
                request.method, request.path
            );

            let mut response = Response::new(&conn.metadata, Status::Ok, b"", ContentType::TEXT);
            if let Some(handler_fn) = router.route(&mut request) {
                handler_fn(&request, &mut response);
            } else {
                match Self::handle_static(request.path, assets_path, &mut response) {
                    Ok(true) => {}
                    Ok(false) => {
                        response.status(Status::NotFound);
                    }
                    Err(e) => return Err(e),
                }
            }

            // let han = if let Some(handlerFn) = router.route(&mut request) {
            //     handlerFn(&request, &mut response);
            //     // resp
            //     // } else {
            //     // None
            // };
            // let socket = conn.socket.try_clone();
            // println!("Socket {:?}", socket.as_ref());
            // let metadata = conn.metadata.;

            // let metadata = &mut conn.metadata;
            // let response = if let Some(resp) = router.route(&mut request, &conn.metadata) {
            //     trace!("Route matched for path: {}", request.path);
            //     resp
            // } else if let Some(resp) =
            //     Self::handle_static(request.path, assets_path, &conn.metadata)
            // {
            //     trace!("Static asset found: {}", request.path);
            //     resp
            // } else {
            //     // let socket = conn.socket.try_clone();

            //     warn!("Route not found: {}", request.path);
            //     Response::new(
            //         &conn.metadata,
            //         Status::NotFound,
            //         "Not Found",
            //         ContentType::TEXT,
            //     )
            //     // Response {
            //     //     conn.metadata,
            //     //     status: Status::Ok,
            //     //     body: "Not Found",
            //     //     content_type: ContentType::TEXT,
            //     //     headers: HashMap::new(),
            //     // }

            //     //  Some(Response {
            //     //     status: Status::Ok,
            //     //     body: "Not Found",
            //     //     ContentType::TEXT,
            //     //     headers: HashMap::new(),
            //     // })
            // };

            // Prepare the response for writing and clear the read buffer to prepare for next cycle.
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

    fn handle_static<'a>(
        path: &str,
        assets_path: &Path,
        response: &mut Response,
    ) -> io::Result<bool> {
        let requested_path = Path::new(path);

        // Prevent directory traversal attacks (e.g., "/../etc/passwd")
        if requested_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            warn!(
                "Security warning: Attempted directory traversal attack with path: {}",
                path
            );
            return Ok(false);
        }

        let mut full_path =
            assets_path.join(requested_path.strip_prefix("/").unwrap_or(requested_path));

        if full_path.is_dir() {
            // If a directory is requested, default to index.html as per standard web server behavior
            full_path.push("index.html");
        }

        if !full_path.is_file() {
            debug!("Static file not found: {}", full_path.display());
            return Ok(false);
        }

        // Attempt to read the file from the filesystem
        match fs::read(&full_path) {
            Ok(content) => {
                let content_type = ContentType::get_content_type(&full_path);
                trace!(
                    "Successfully read static file: {} ({} bytes)",
                    full_path.display(),
                    content.len()
                );

                response.body(content);
                response.content_type(content_type);
                // Some(Response {
                //     metadata,
                //     status: Status::Ok,
                //     body: content,
                //     content_type,
                //     headers: HashMap::new(),
                // })
                return Ok(true);
            }
            Err(err) => {
                // Log as error because a file that 'should' exist but can't be read
                // is a filesystem issue (permissions, locked files, etc.)
                error!("Failed to read static file {:?}: {}", full_path, err);
                // None
                return Ok(false);
            }
        }
    }

    pub fn assets_path(&mut self, path: &str) -> &mut Self {
        self.assets_path = PathBuf::from(path);
        self
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

        let mut connections: HashMap<i32, Connection<TcpStream>> = HashMap::new();

        let startup_us = self.init_start.elapsed().as_micros();
        let local_addr = self.listener.local_addr()?;
        // info! is perfect here; it's a one-time startup event.
        info!(
            "Server started on http://{} in {}µs",
            local_addr, startup_us
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
                    -1, // 2-second timeout
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

            // Handle listener (index 0)
            if poll_fds[0].revents & POLLIN != 0 {
                loop {
                    match self.listener.accept() {
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

            // Handle client connections
            for (i, item) in poll_fds.iter_mut().enumerate().skip(1) {
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
                                item.events = POLLIN;
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
                } else if revents & POLLIN != 0
                    && let Some(conn) = connections.get_mut(&fd)
                {
                    match Self::handle_read(conn, &self.router, &self.assets_path) {
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
                    // }
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
