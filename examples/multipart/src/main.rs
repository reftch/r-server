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

            for field in fields {
                match field.filename {
                    Some(filename) => {
                        info!("saving file {filename} ({} bytes)", field.data.len());
                        std::fs::write(&filename, &field.data).expect("failed to write file");
                    }
                    None => {
                        info!(
                            "field `{}` = {}",
                            field.name,
                            String::from_utf8_lossy(&field.data)
                        );
                    }
                }
            }

            res.body("Uploaded");
        })
        .run()?;
    Ok(())
}
