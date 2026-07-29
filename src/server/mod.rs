pub mod connection;
pub mod http;
pub mod https;

// use crate::router::{HandlerFn, Method};

// pub struct ServerBuilder<T> {
//     server: T,
// }

// impl<T> ServerBuilder<T> {
//     pub fn new() -> Self {
//         // Self { server: T }
//     }

//     /// Adds a route to the server
//     pub fn route(mut self, method: Method, path: &str, handler: HandlerFn<T>) -> Self {
//         // if let Some(ref mut st) = self.server_type {
//         //     let success = match st {
//         //         ServerType::Standard(s) => s.route(method, path, handler<TcpStream>),
//         //         ServerType::Secure(s) => s.route(method, path, handler),
//         //     };
//         //     if !success {
//         //         // In a real world scenario, we might want to return a Result or log a warning.
//         //         // For now, we just proceed.
//         //     }
//         // }
//         self
//     }

//     // The final step that returns something capable of running
//     pub fn run(self) -> std::io::Result<()> {
//         Ok(())
//     }
// }
