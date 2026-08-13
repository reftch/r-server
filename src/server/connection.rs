use std::io;

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
