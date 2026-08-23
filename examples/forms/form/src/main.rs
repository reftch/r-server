use r_server::{core::http::Server, response::Status, router::Method};

fn main() -> std::io::Result<()> {
    Server::new()?
        .route(Method::GET, "/", |_req, res| {
            res.content_type(r_server::response::ContentType::HTML)
                .body(
                    r#"<html>
                        <head><title>Upload Test</title></head>
                        <body>
                            <h3>Will hit handle post</h3>
                            <form action=/post method=POST>
                                <label for="name">Name:</label>
                                <input name="name">
                                <button type=submit>Submit form</button>
                            </form>
                        </body>
                    </html>"#,
                );
        })
        .route(Method::POST, "/post", |req, res| {
            match req.get_form_field("name") {
                Ok(name) => {
                    res.body(format!("Your name is {}", name));
                }
                Err(e) => {
                    res.status(Status::BadRequest);
                    res.body(e);
                }
            }
        })
        .run()?;

    Ok(())
}
