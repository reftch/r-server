use r_server::info;

fn main() {
    info!("Hello from example!");
    std::thread::sleep(std::time::Duration::from_millis(100));
}
