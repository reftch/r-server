use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_would_block() {
        let error = std::io::Error::new(std::io::ErrorKind::WouldBlock, "would block");
        assert!(Server::would_block(&error));

        let interrupt = std::io::Error::new(std::io::ErrorKind::Interrupted, "interrupted");
        assert!(Server::would_block(&interrupt));

        let real_error = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "reset");
        assert!(!Server::would_block(&real_error));
    }
}
