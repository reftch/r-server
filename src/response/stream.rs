use std::io::{self, Write};
use std::net::TcpStream;

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
