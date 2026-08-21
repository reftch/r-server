use r_server::{core::http::Server, info, router::Method};

fn main() -> std::io::Result<()> {
    Server::new()?
        .route(Method::GET, "/", |_req, res| {
            res.content_type(r_server::response::ContentType::HTML)
                .body(
                    r#"<html>
                        <head><title>Upload Test</title></head>
                        <body>
                            <form target="/" method="post" enctype="multipart/form-data">
                                <input type="file" multiple name="file"/>
                                <button type="submit">Submit</button>
                            </form>
                        </body>
                    </html>"#,
                );
        })
        .route(Method::POST, "/", |_req, res| {
            info!("uploaded");
        })
        .run()?;
    Ok(())
}
