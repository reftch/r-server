use r_server::{core::http::Server, info, response::Status, router::Method};

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
            match req.get_form_file("file") {
                Ok(file) => {
                    let filename = file.filename.unwrap_or_else(|| "upload.bin".into());
                    info!("saving {filename} ({} bytes)", file.data.len());
                    std::fs::write(&filename, &file.data).expect("failed to write file");
                    res.body("Uploaded");
                }
                Err(e) => {
                    res.status(Status::BadRequest).body(e);
                }
            }
        })
        .run()?;
    Ok(())
}
