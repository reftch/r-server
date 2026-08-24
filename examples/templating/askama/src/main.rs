use r_server::{core::http::Server, response::ContentType, router::Method};

use askama::Template;

#[derive(Template)]
#[template(path = "user.html")]
struct UserTemplate<'a> {
    name: &'a str,
    text: &'a str,
}

#[derive(Template)]
#[template(path = "index.html")]
struct Index;

fn main() -> std::io::Result<()> {
    Server::new()?
        .route(Method::GET, "/", |req, res| {
            let html = if let Some(name) = req.query("name") {
                UserTemplate {
                    name,
                    text: "Welcome!",
                }
                .render()
                .expect("template should be valid")
            } else {
                Index.render().expect("template should be valid")
            };

            res.content_type(ContentType::HTML).body(html);
        })
        .assets_path(concat!(env!("CARGO_MANIFEST_DIR"), "/templates"))
        .run()
}
