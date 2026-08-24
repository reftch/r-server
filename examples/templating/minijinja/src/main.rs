use std::error::Error;
use std::sync::OnceLock;

use minijinja::{Environment, context};
use r_server::{
    core::http::Server,
    request::Request,
    response::{ContentType, Response},
    router::Method,
};

static JINJA_ENV: OnceLock<Environment<'static>> = OnceLock::new();

fn handle_index(req: &Request, res: &mut Response) {
    let env = JINJA_ENV
        .get()
        .expect("MiniJinja environment not initialized");

    let html = if let Some(name) = req.query("name") {
        let tmpl = env
            .get_template("user.html")
            .expect("Failed to get user template");

        tmpl.render(context! {
            name => name,
            text => "Welcome!",
        })
        .expect("Failed to render user template")
    } else {
        let tmpl = env
            .get_template("index.html")
            .expect("Failed to get index template");

        tmpl.render(context! {})
            .expect("Failed to render index template")
    };

    res.content_type(ContentType::HTML).body(html);
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut env = Environment::new();

    // Load templates from disk at startup
    env.set_loader(minijinja::path_loader(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/templates"
    )));

    JINJA_ENV
        .set(env)
        .ok()
        .expect("Failed to store MiniJinja environment");

    Server::new()?
        .route(Method::GET, "/", handle_index)
        .assets_path(concat!(env!("CARGO_MANIFEST_DIR"), "/templates"))
        .run()?;

    Ok(())
}
