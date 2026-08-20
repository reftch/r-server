#[cfg(test)]
mod tests {
    use crate::logger;
    use crate::logger::LogLevel;
    use crate::request::Request;
    use crate::response::Response;
    use crate::router::Method;
    use crate::server::http::Server;

    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::Mutex;
    use std::thread;
    use std::time::Duration;

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
}
