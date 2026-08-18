use std::io;

use crate::response::stream::StreamWriter;

#[derive(Debug, Clone)]
pub struct ConnectionMetadata<T> {
    pub stream: T,
}

pub trait ConnectionStreamClone {
    fn clone_stream(&self) -> io::Result<Self>
    where
        Self: Sized;
}

impl<T> ConnectionMetadata<T>
where
    T: ConnectionStreamClone,
{
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            stream: self.stream.clone_stream()?,
        })
    }
}

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
