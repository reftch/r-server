use std::error::Error;

use r_server::{
    core::http::Server,
    request::Request,
    response::{ContentType, Response},
    router::Method,
};
use sailfish::TemplateOnce;

#[derive(TemplateOnce)]
#[template(path = "user.html")]
struct UserTemplate<'a> {
    name: &'a str,
    text: &'a str,
}

#[derive(TemplateOnce)]
#[template(path = "index.html")]
struct IndexTemplate;

fn handle_index(req: &Request, res: &mut Response) {
    let html = if let Some(name) = req.query("name") {
        UserTemplate {
            name,
            text: "Welcome!",
        }
        .render_once()
        .expect("Failed to render user template")
    } else {
        IndexTemplate
            .render_once()
            .expect("Failed to render index template")
    };

    res.content_type(ContentType::HTML).body(html);
}

fn main() -> Result<(), Box<dyn Error>> {
    Server::new()?
        .route(Method::GET, "/", handle_index)
        .assets_path(concat!(env!("CARGO_MANIFEST_DIR"), "/templates"))
        .run()?;

    Ok(())
}
