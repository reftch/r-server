use r_server::{
    core::http::Server,
    request::Request,
    response::{ContentType, Response, Status},
    router::Method,
};

fn index(req: &Request, res: &mut Response) {
    let user = req.session().and_then(|s| s.get("user_id"));

    let page = match user {
        Some(user) => format!("<h1>Hello, {user}</h1><a href=\"/logout\">Log out</a>"),
        None => "<h1>Guest</h1>\
                 <form method=\"POST\" action=\"/login\">\
                 <input name=\"username\" placeholder=\"username\">\
                 <button type=\"submit\">Log in</button>\
                 </form>"
            .to_string(),
    };

    res.content_type(ContentType::HTML).body(page);
}

fn whoami(req: &Request, res: &mut Response) {
    match req.session().and_then(|s| s.get("user_id")) {
        Some(user) => {
            res.content_type(ContentType::JSON)
                .body(format!("{{\"user\":\"{user}\"}}"));
        }
        None => {
            res.status(Status::Unauthorized).body("no active session");
        }
    }
}

fn login(req: &Request, res: &mut Response) {
    let username = req.get_form_field("username");

    match (username, req.session()) {
        (Ok(username), Some(session)) => {
            // Arbitrary per-session data works too:
            session.set("last_login", "just now");
            session.set("user_id", username);

            // Note: the server skips writing responses with empty bodies, so
            // redirects always carry a short placeholder body.
            res.status(Status::MovedTemporarily)
                .header("Location", "/")
                .body("Redirecting...");
        }
        (_, None) => {
            res.status(Status::InternalServerError)
                .body("sessions are not enabled on this server");
        }
        (Err(_), _) => {
            res.status(Status::BadRequest)
                .body("missing 'username' field");
        }
    }
}

fn logout(req: &Request, res: &mut Response) {
    if let Some(session) = req.session() {
        session.destroy();
    }

    res.status(Status::MovedTemporarily)
        .header("Location", "/")
        .body("Redirecting...");
}

fn main() -> std::io::Result<()> {
    Server::new()?
        .sessions_ttl(3600) // sessions survive 1h of inactivity
        .route(Method::GET, "/", index)
        .route(Method::GET, "/whoami", whoami)
        .route(Method::POST, "/login", login)
        .route(Method::GET, "/logout", logout)
        .route(Method::POST, "/logout", logout)
        .run()?;

    Ok(())
}
