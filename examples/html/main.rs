use std::{thread, time::Duration};

use r_server::{response, router::Method, server::http::Server};

fn main() -> std::io::Result<()> {
    // r_server::logger::set_level(logger::LogLevel::Trace);
    Server::new("127.0.0.1:8082")?
        .route(Method::GET, "/api/v1/users/:id", |req, res| {
            if let Some(id) = req.param("id") {
                res.content_type(response::ContentType::JSON)
                    .body(format!("{{\"value\":{}}}", id));
            }
        })
        .route(Method::GET, "/stream", |_, res| {
            // res.enable_sse();

            // let res_ptr = res.try_into()?;
            let res_for_thread = res;
            thread::spawn(move || {
                let mut i = 0;
                loop {
                    i += 1;
                    let message = format!("data: {}\n\n", i);
                    if let Err(e) = res_for_thread.sse(&message) {
                        eprintln!("SSE connection closed: {}", e);
                        break;
                    }
                    thread::sleep(Duration::from_secs(1));
                    if i >= 100 {
                        break;
                    }
                }
            });
        })
        .assets_path("./examples/html/assets")
        .run()?;

    Ok(())
}
