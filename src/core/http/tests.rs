#[cfg(test)]
mod tests {
    use crate::core::http::Server;
    use crate::logger;
    use crate::logger::LogLevel;
    use crate::request::Request;
    use crate::response::Response;
    use crate::router::Method;

    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::Mutex;
    use std::thread;
    use std::time::{Duration, Instant};

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn hello_handler(_req: &Request, res: &mut Response) {
        res.body("Hello, World!");
    }

    #[test]
    fn test_server_connection() -> Result<(), Box<dyn std::error::Error>> {
        let _guard = TEST_LOCK.lock().unwrap();
        logger::set_level(LogLevel::None);

        let mut server = Server::new()?;
        server
            .bind("127.0.0.1", 18080)
            .route(Method::GET, "/", hello_handler);

        thread::spawn(move || {
            if let Err(e) = server.run() {
                eprintln!("Server error: {}", e);
            }
        });

        thread::sleep(Duration::from_millis(100));

        let mut stream = TcpStream::connect("127.0.0.1:18080")?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;

        stream.write_all(
            b"GET / HTTP/1.1\r\n\
              Host: 127.0.0.1\r\n\
              Connection: close\r\n\
              \r\n",
        )?;

        let mut buffer = Vec::new();
        let mut chunk = [0u8; 1024];

        loop {
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    buffer.extend_from_slice(&chunk[..n]);

                    if buffer.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    break;
                }
                Err(e) => return Err(e.into()),
            }
        }

        let response = String::from_utf8_lossy(&buffer);

        assert!(response.contains("HTTP/1.1"), "Response: {response}");
        assert!(response.contains("200 OK"), "Response: {response}");
        assert!(response.contains("Hello, World!"), "Response: {response}");

        Ok(())
    }

    #[test]
    fn test_server_404() -> Result<(), Box<dyn std::error::Error>> {
        let _guard = TEST_LOCK.lock().unwrap();
        logger::set_level(LogLevel::None);

        let mut server = Server::new()?;
        server
            .bind("127.0.0.1", 18081)
            .route(Method::GET, "/", hello_handler);

        thread::spawn(move || {
            if let Err(e) = server.run() {
                eprintln!("Server error: {}", e);
            }
        });

        thread::sleep(Duration::from_millis(100));

        let mut stream = TcpStream::connect("127.0.0.1:18081")?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;

        stream.write_all(
            b"GET /not-found HTTP/1.1\r\n\
              Host: 127.0.0.1\r\n\
              Connection: close\r\n\
              \r\n",
        )?;

        let mut buffer = Vec::new();
        let mut chunk = [0u8; 1024];

        loop {
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    buffer.extend_from_slice(&chunk[..n]);

                    if buffer.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    break;
                }
                Err(e) => return Err(e.into()),
            }
        }

        let response = String::from_utf8_lossy(&buffer);

        assert!(response.contains("404 Not Found"), "Response: {response}");

        Ok(())
    }

    #[test]
    fn test_server_multi_worker() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _guard = TEST_LOCK.lock().unwrap();
        logger::set_level(LogLevel::None);

        let mut server = Server::new()?;
        server
            .bind("127.0.0.1", 18082)
            .workers(4)
            .route(Method::GET, "/", hello_handler);

        thread::spawn(move || {
            if let Err(e) = server.run() {
                eprintln!("Server error: {}", e);
            }
        });

        thread::sleep(Duration::from_millis(100));

        let fetch = |id: u8| -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
            let mut stream = TcpStream::connect("127.0.0.1:18082")?;
            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
            stream.set_write_timeout(Some(Duration::from_secs(2)))?;

            write!(
                stream,
                "GET /?id={id} HTTP/1.1\r\n\
                 Host: 127.0.0.1\r\n\
                 Connection: close\r\n\
                 \r\n"
            )?;

            let mut buffer = Vec::new();
            let mut chunk = [0u8; 1024];

            loop {
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        buffer.extend_from_slice(&chunk[..n]);
                        if buffer.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                    Err(e) => return Err(e.into()),
                }
            }

            Ok(String::from_utf8_lossy(&buffer).into_owned())
        };

        // Sequential requests must all be served regardless of which worker accepts.
        for id in 0..16u8 {
            let response = fetch(id)?;
            assert!(response.contains("200 OK"), "Request {id}: {response}");
            assert!(
                response.contains("Hello, World!"),
                "Request {id}: {response}"
            );
        }

        // Concurrent requests across workers.
        let handles: Vec<_> = (0..8u8)
            .map(|id| thread::spawn(move || fetch(id)))
            .collect();

        for handle in handles {
            let response = handle.join().unwrap()?;
            assert!(response.contains("Hello, World!"), "Concurrent: {response}");
        }

        Ok(())
    }

    /// Spawns `server.run()` on a background thread and waits until the
    /// listening socket accepts connections (and signal handlers are
    /// installed, which happens before the event loop starts).
    fn start_server(mut server: Server, addr: &'static str) -> thread::JoinHandle<()> {
        let handle = thread::spawn(move || {
            if let Err(e) = server.run() {
                eprintln!("Server error: {}", e);
            }
        });

        let mut connected = false;
        for _ in 0..150 {
            if TcpStream::connect(addr).is_ok() {
                connected = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(connected, "server did not start on {addr}");

        // A successful connect proves signal handlers are already installed:
        // run() installs them before binding the listening socket.
        handle
    }

    /// Waits until connecting to `addr` is refused, proving the listener
    /// socket was released during shutdown.
    fn assert_port_released(addr: &'static str) {
        for _ in 0..250 {
            match TcpStream::connect(addr) {
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => return,
                _ => thread::sleep(Duration::from_millis(20)),
            }
        }
        panic!("listening socket on {addr} was never released");
    }

    /// Reads from `stream` until EOF or timeout; returns elapsed time and
    /// everything received.
    fn read_until_eof(
        stream: &mut TcpStream,
    ) -> Result<(Duration, Vec<u8>), Box<dyn std::error::Error + Send + Sync>> {
        let start = Instant::now();
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 512];

        loop {
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => buffer.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => break,
                Err(e) => return Err(e.into()),
            }
        }

        Ok((start.elapsed(), buffer))
    }

    #[test]
    fn test_graceful_shutdown_flushes_in_flight_response()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _guard = TEST_LOCK.lock().unwrap();
        logger::set_level(LogLevel::None);

        let addr: &'static str = "127.0.0.1:18083";
        let mut server = Server::new()?;
        server
            .bind("127.0.0.1", 18083)
            .workers(1)
            .shutdown_timeout(Duration::from_secs(5))
            .route(Method::GET, "/slow", |_req, res| {
                thread::sleep(Duration::from_millis(300));
                res.body("Slow Response");
            });

        let server_handle = start_server(server, addr);

        // Idle keep-alive connection: must be closed as soon as shutdown
        // starts instead of being held until the drain timeout.
        let mut idle = TcpStream::connect(addr)?;
        idle.set_read_timeout(Some(Duration::from_secs(5)))?;

        // In-flight request: its response must be flushed before closing.
        let mut active = TcpStream::connect(addr)?;
        active.set_read_timeout(Some(Duration::from_secs(5)))?;
        active.set_write_timeout(Some(Duration::from_secs(5)))?;
        active.write_all(b"GET /slow HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")?;

        // The handler is now sleeping mid-request.
        thread::sleep(Duration::from_millis(100));

        // Trigger graceful shutdown through the real signal path.
        unsafe {
            libc::kill(libc::getpid(), libc::SIGINT);
        }

        // The idle connection should hit EOF promptly.
        let (elapsed, _) = read_until_eof(&mut idle)?;
        assert!(
            elapsed < Duration::from_secs(3),
            "idle connection took {:?} to close",
            elapsed
        );

        // The in-flight connection receives its complete response.
        let (elapsed, received) = read_until_eof(&mut active)?;
        let response = String::from_utf8_lossy(&received);
        assert!(
            response.contains("200 OK") && response.contains("Slow Response"),
            "in-flight response lost after {:?}: {response}",
            elapsed
        );

        assert_port_released(addr);
        server_handle.join().unwrap();

        Ok(())
    }

    #[test]
    fn test_shutdown_completes_in_flight_handler_response()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _guard = TEST_LOCK.lock().unwrap();
        logger::set_level(LogLevel::None);

        let addr: &'static str = "127.0.0.1:18084";
        let mut server = Server::new()?;
        server
            .bind("127.0.0.1", 18084)
            .workers(1)
            .shutdown_timeout(Duration::from_secs(30))
            .route(Method::GET, "/slow-handler", |_req, res| {
                thread::sleep(Duration::from_millis(600));
                res.body("Late Response");
            });

        let server_handle = start_server(server, addr);

        let mut conn = TcpStream::connect(addr)?;
        conn.set_read_timeout(Some(Duration::from_secs(10)))?;
        conn.set_write_timeout(Some(Duration::from_secs(5)))?;
        conn.write_all(b"GET /slow-handler HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")?;

        // Handler still sleeping when shutdown arrives.
        thread::sleep(Duration::from_millis(100));

        unsafe {
            libc::kill(libc::getpid(), libc::SIGINT);
        }

        // Handlers run synchronously on the event loop, so shutdown waits
        // for the one in flight; its response must still be delivered in
        // full rather than truncated.
        let (_, received) = read_until_eof(&mut conn)?;
        let response = String::from_utf8_lossy(&received);
        assert!(
            response.contains("200 OK") && response.contains("Late Response"),
            "in-flight response lost: {response}"
        );

        assert_port_released(addr);
        server_handle.join().unwrap();

        Ok(())
    }
}
