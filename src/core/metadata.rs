use crate::core::connection::ConnectionMetadata;
use std::io::Write;

pub trait Metadata: Send + Sync {
    fn write(&self, buf: &[u8]) -> std::io::Result<()>;

    /// Creates a cloned trait object.
    fn clone_box(&self) -> Box<dyn Metadata>;
}

impl<S> Metadata for ConnectionMetadata<S>
where
    S: Send + Sync + 'static,
    for<'a> &'a S: Write, // Ensures &S implements Write (satisfied by TcpStream)
{
    fn write(&self, buf: &[u8]) -> std::io::Result<()> {
        // Dereference Arc to call Write on &S and convert Result<usize> to Result<()>
        (&*self.stream).write_all(buf)
    }

    fn clone_box(&self) -> Box<dyn Metadata> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Metadata> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
