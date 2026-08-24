use std::cell::RefCell;
use std::error::Error;

use r_server::{
    core::http::Server,
    request::Request,
    response::{ContentType, Response},
    router::Method,
};
use serde_json::json;
use tinytemplate::TinyTemplate;

thread_local! {
    static TINY_TT: RefCell<TinyTemplate<'static>> = RefCell::new({
        let mut tt = TinyTemplate::new();

        let index_src: &'static str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/templates/index.html"
        ));
        let user_src: &'static str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/templates/user.html"
        ));

        tt.add_template("index", index_src).expect("Failed to register index template");
        tt.add_template("user", user_src).expect("Failed to register user template");

        tt
    });
}

fn handle_index(req: &Request, res: &mut Response) {
    let html = TINY_TT.with(|tt_cell| {
        let tt = tt_cell.borrow();

        if let Some(name) = req.query("name") {
            tt.render(
                "user",
                &json!({
                    "name": name,
                    "text": "Welcome!",
                }),
            )
            .expect("Failed to render user template")
        } else {
            tt.render("index", &json!({}))
                .expect("Failed to render index template")
        }
    });

    res.content_type(ContentType::HTML).body(html);
}

fn main() -> Result<(), Box<dyn Error>> {
    Server::new()?
        .route(Method::GET, "/", handle_index)
        .assets_path(concat!(env!("CARGO_MANIFEST_DIR"), "/templates"))
        .run()?;

    Ok(())
}
