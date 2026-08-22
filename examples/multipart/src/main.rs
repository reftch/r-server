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
        .route(Method::POST, "/", |req, res| {
            let fields = req
                .get_multipart_fields()
                .expect("failed to parse multipart");

            if let Some(first) = fields.first() {
                let path = format!("{}", first.filename.as_deref().unwrap_or("un"));
                info!("saving to {path}");
                std::fs::write(&path, &first.data).expect("failed to write file");
            }
            res.body("Uploaded");
        })
        .run()?;
    Ok(())
}
