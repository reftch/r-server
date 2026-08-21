use r_server::server::http::Server;

fn main() -> std::io::Result<()> {
    Server::new()?.bind("0.0.0.0", 8080).run()?;
    Ok(())
}
