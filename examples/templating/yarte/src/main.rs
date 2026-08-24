use std::error::Error;

// Import the Template trait so the .call() method is available
use yarte::Template;

use r_server::{
    core::http::Server,
    request::Request,
    response::{ContentType, Response},
    router::Method,
};

#[derive(Template)]
#[template(path = "user.hbs")]
struct UserTemplate<'a> {
    name: &'a str,
    text: &'a str,
}

#[derive(Template)]
#[template(path = "index.hbs")]
struct IndexTemplate;

fn handle_index(req: &Request, res: &mut Response) {
    let html = if let Some(name) = req.query("name") {
        UserTemplate {
            name,
            text: "Welcome!",
        }
        .call()
        .expect("Failed to render user template")
    } else {
        IndexTemplate
            .call()
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
