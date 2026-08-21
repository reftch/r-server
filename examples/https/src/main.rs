use r_server::server::https::Server;

fn main() -> std::io::Result<()> {
    Server::new()?.run()?;
    Ok(())
}
