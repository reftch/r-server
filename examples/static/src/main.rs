use r_server::{core::http::Server, router::Method};

fn main() -> std::io::Result<()> {
    Server::new()?
        .route(Method::GET, "/dashboard", |_req, res| {
            res.content_type(r_server::response::ContentType::HTML)
                .body(
                    r#"<html>
                        <head>
                            <title>Dashboard page</title>
                            <link rel="stylesheet" href="styles.css" />
                        </head>
                        <div class="container">
                            <nav>
                                <a href="/">Index page</a>
                                <a href="/home">Home page</a>
                            </nav>
                        </div>
                        <body>
                        </body>
                    </html>"#,
                );
        })
        .assets_path("./examples/static/assets")
        .workers(2)
        .run()
}
