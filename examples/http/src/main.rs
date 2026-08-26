use r_server::core::http::Server;

fn main() -> std::io::Result<()> {
    Server::new()?.run()
}
