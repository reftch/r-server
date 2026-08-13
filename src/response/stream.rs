use std::io::{self, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

use crate::server::https::TlsState;

pub trait StreamWriter {
    fn write(&self, data: &[u8]) -> io::Result<()>;
}

impl StreamWriter for TcpStream {
    fn write(&self, data: &[u8]) -> io::Result<()> {
        let mut stream = self.try_clone()?;
        stream.write_all(data)
    }
}

impl StreamWriter for Option<TlsState> {
    fn write(&self, data: &[u8]) -> io::Result<()> {
        match self {
            Some(TlsState::Connected(stream)) => {
                let mut stream = stream.get_ref().try_clone()?;
                stream.write_all(data)
            }

            Some(TlsState::Handshaking(stream)) => {
                let mut stream = stream.get_ref().try_clone()?;
                stream.write_all(data)
            }

            None => Ok(()),
        }
    }
}

impl StreamWriter for Option<Arc<Mutex<Option<TlsState>>>> {
    fn write(&self, data: &[u8]) -> io::Result<()> {
        let shared = self.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "TLS stream is not initialized")
        })?;

        let mut state = shared
            .lock()
            .map_err(|_| io::Error::other("TLS stream mutex poisoned"))?;

        match state.as_mut() {
            Some(TlsState::Connected(stream)) => stream.write_all(data),

            Some(TlsState::Handshaking(_)) => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "TLS handshake is not complete",
            )),

            None => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "TLS state is not initialized",
            )),
        }
    }
}
