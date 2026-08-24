use std::error::Error;
use std::sync::OnceLock;

use handlebars::Handlebars;
use r_server::{
    core::http::Server,
    request::Request,
    response::{ContentType, Response},
    router::Method,
};
use serde_json::json;

// Global thread-safe static container for the template registry
static HANDLEBARS: OnceLock<Handlebars> = OnceLock::new();

fn handle_index(req: &Request, res: &mut Response) {
    let reg = HANDLEBARS.get().expect("Handlebars should be initialized");

    let html = if let Some(name) = req.query("name") {
        reg.render(
            "user",
            &json!({
                "name": name,
                "text": "Welcome!"
            }),
        )
        .expect("template should be valid")
    } else {
        reg.render("index", &json!({}))
            .expect("template should be valid")
    };

    res.content_type(ContentType::HTML).body(html);
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut reg = Handlebars::new();

    // Register templates from files
    reg.register_template_file(
        "index",
        concat!(env!("CARGO_MANIFEST_DIR"), "/templates/index.html"),
    )?;
    reg.register_template_file(
        "user",
        concat!(env!("CARGO_MANIFEST_DIR"), "/templates/user.html"),
    )?;

    // Store in global static state before starting the server
    HANDLEBARS
        .set(reg)
        .ok()
        .expect("Failed to initialize Handlebars registry");

    Server::new()?
        .route(Method::GET, "/", handle_index)
        .assets_path(concat!(env!("CARGO_MANIFEST_DIR"), "/templates"))
        .run()?;

    Ok(())
}
