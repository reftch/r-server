use std::{io, sync::Arc};

pub struct ConnectionMetadata<S> {
    pub stream: Arc<S>,
}

impl<S> Clone for ConnectionMetadata<S> {
    fn clone(&self) -> Self {
        Self {
            // Clones the Arc reference count, not the underlying stream `S`
            stream: Arc::clone(&self.stream),
        }
    }
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
            // 1. Call clone_stream() on inner `T` via .as_ref()
            // 2. Wrap the cloned stream back into an Arc
            stream: Arc::new(self.stream.as_ref().clone_stream()?),
        })
    }
}
