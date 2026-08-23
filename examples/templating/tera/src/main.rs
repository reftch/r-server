use r_server::{
    core::http::Server,
    response::{ContentType, Status},
    router::Method,
};
use std::sync::LazyLock;
use tera::{Context, Tera};

static TERA: LazyLock<Tera> = LazyLock::new(|| {
    let mut tera = Tera::default();
    tera.load_from_glob(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/**/*.html"))
        .expect("Failed to load templates");
    tera
});

fn main() -> std::io::Result<()> {
    Server::new()?
        .route(Method::GET, "/", |req, res| {
            let mut context = Context::new();
            let template_name = if let Some(name) = req.query("name") {
                context.insert("name", name);
                context.insert("text", "Welcome!");
                "user.html"
            } else {
                "index.html"
            };

            match TERA.render(template_name, &context) {
                Ok(html) => {
                    res.content_type(ContentType::HTML).body(html);
                }
                Err(e) => {
                    eprintln!("Template render error: {e}");
                    res.status(Status::InternalServerError)
                        .body("Internal Server Error".to_string());
                }
            }
        })
        .assets_path(concat!(env!("CARGO_MANIFEST_DIR"), "/templates"))
        .run()
}
