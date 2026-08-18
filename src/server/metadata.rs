use std::io;

use crate::{
    response::stream::StreamWriter,
    server::connection::{ConnectionMetadata, ConnectionStreamClone},
};

pub trait Metadata: Send + Sync {
    fn try_clone_metadata(&self) -> io::Result<Box<dyn Metadata>>;
    fn write(&self, data: &[u8]) -> io::Result<()>;
}

impl<T> Metadata for ConnectionMetadata<T>
where
    T: ConnectionStreamClone + StreamWriter + Send + Sync + 'static,
{
    fn try_clone_metadata(&self) -> io::Result<Box<dyn Metadata>> {
        Ok(Box::new(self.try_clone()?))
    }

    fn write(&self, data: &[u8]) -> io::Result<()> {
        self.stream.write(data)
    }
}
