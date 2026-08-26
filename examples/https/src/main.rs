use r_server::core::https::Server;

fn main() -> std::io::Result<()> {
    Server::new()?.run()
}
