use askama::Template;
use r_server::{
    client::Client,
    core::http::Server,
    request::Request,
    response::{ContentType, Response, Status},
    router::Method,
};
use serde::{Deserialize, Serialize};

const DEFAULT_REDIRECT_URI: &str = "http://localhost:8080/auth/google/callback";
const USER_SESSION_KEY: &str = "google_user";

fn client_id() -> String {
    std::env::var("GOOGLE_KEY").expect("GOOGLE_KEY must be set in the environment or a .env file")
}

fn redirect_uri() -> String {
    std::env::var("GOOGLE_CALLBACK").unwrap_or_else(|_| DEFAULT_REDIRECT_URI.to_string())
}

#[derive(Template)]
#[template(path = "index.html")]
struct Index<'a> {
    user: Option<UserView<'a>>,
    client_id: &'a str,
    redirect_uri: &'a str,
}

struct UserView<'a> {
    name: &'a str,
    picture: Option<&'a str>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Serialize, Deserialize)]
struct GoogleUser {
    sub: String,
    name: Option<String>,
    email: Option<String>,
    picture: Option<String>,
}

fn current_user(req: &Request) -> Option<GoogleUser> {
    let raw = req.session()?.get(USER_SESSION_KEY)?;
    serde_json::from_str(&raw).ok()
}

fn index(req: &Request, res: &mut Response) {
    let g_user = current_user(req);
    let user = g_user.as_ref().map(|u| UserView {
        name: u.name.as_deref().unwrap_or(&u.sub),
        picture: u.picture.as_deref(),
    });

    let client_id = client_id();
    let redirect_uri = redirect_uri();

    let html = Index {
        user,
        client_id: &client_id,
        redirect_uri: &redirect_uri,
    }
    .render()
    .expect("template should be valid");

    res.content_type(ContentType::HTML).body(html);
}

fn logout(req: &Request, res: &mut Response) {
    if let Some(session) = req.session() {
        session.destroy();
    }

    res.status(Status::MovedTemporarily)
        .header("Location", "/")
        .body("Redirecting...");
}

fn callback(req: &Request, res: &mut Response) {
    let Some(code) = req.query("code") else {
        res.status(Status::BadRequest)
            .body("Missing code query parameter");
        return;
    };

    let token_body = format!(
        "code={}&client_id={}&client_secret={}&redirect_uri={}&grant_type=authorization_code",
        code,
        client_id(),
        std::env::var("GOOGLE_SECRET").unwrap_or_default(),
        redirect_uri()
    );

    let auth = Client::new("https://oauth2.googleapis.com");
    let token_resp_body = match auth.post_with(
        "/token",
        token_body,
        &[("Content-Type", "application/x-www-form-urlencoded")],
    ) {
        Ok(body) => body,
        Err(_) => {
            res.status(Status::InternalServerError)
                .body("Failed to communicate with Google");
            return;
        }
    };

    let token_data: TokenResponse = match serde_json::from_str(&token_resp_body) {
        Ok(data) => data,
        Err(_) => {
            res.status(Status::BadRequest)
                .body("Invalid or expired authorization code");
            return;
        }
    };

    let api = Client::new("https://www.googleapis.com");
    let user_body = match api.get_with(
        "/oauth2/v3/userinfo",
        &[(
            "Authorization",
            &format!("Bearer {}", token_data.access_token),
        )],
    ) {
        Ok(body) => body,
        Err(_) => {
            res.status(Status::InternalServerError)
                .body("Failed to fetch user profile from Google");
            return;
        }
    };

    let g_user: GoogleUser = match serde_json::from_str(&user_body) {
        Ok(user) => user,
        Err(_) => {
            res.status(Status::InternalServerError)
                .body("Failed to parse user profile response");
            return;
        }
    };

    let Some(session) = req.session() else {
        res.status(Status::InternalServerError)
            .body("sessions are not enabled on this server");
        return;
    };

    let Ok(profile_json) = serde_json::to_string(&g_user) else {
        res.status(Status::InternalServerError)
            .body("Failed to serialize user profile");
        return;
    };
    session.set(USER_SESSION_KEY, profile_json);

    res.status(Status::MovedTemporarily)
        .header("Location", "/")
        .body("Redirecting...");
}

fn main() -> std::io::Result<()> {
    // Load from the example directory regardless of where `cargo run` was
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/.env")).ok();

    Server::new()?
        .sessions_ttl(-1)
        .route(Method::GET, "/", index)
        .route(Method::GET, "/auth/google/callback", callback)
        .route(Method::POST, "/logout", logout)
        .route(Method::GET, "/logout", logout)
        .assets_path(concat!(env!("CARGO_MANIFEST_DIR"), "/templates"))
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_user() -> GoogleUser {
        GoogleUser {
            sub: "110169484474386276334".into(),
            name: Some("Jane Doe".into()),
            email: Some("jane@example.com".into()),
            picture: Some("https://lh3.googleusercontent.com/a/photo.jpg".into()),
        }
    }

    #[test]
    fn user_survives_session_roundtrip() {
        let raw = serde_json::to_string(&sample_user()).unwrap();
        let restored: GoogleUser = serde_json::from_str(&raw).unwrap();

        assert_eq!(restored.sub, "110169484474386276334");
        assert_eq!(restored.name.as_deref(), Some("Jane Doe"));
        assert_eq!(restored.email.as_deref(), Some("jane@example.com"));
        assert_eq!(
            restored.picture.as_deref(),
            Some("https://lh3.googleusercontent.com/a/photo.jpg")
        );
    }

    #[test]
    fn index_renders_logged_in_user_from_session_data() {
        let u = sample_user();
        let html = Index {
            user: Some(UserView {
                name: u.name.as_deref().unwrap_or(&u.sub),
                picture: u.picture.as_deref(),
            }),
            client_id: "k",
            redirect_uri: "r",
        }
        .render()
        .unwrap();

        assert!(html.contains("Hi, Jane Doe!"));
        assert!(html.contains(r#"src="https://lh3.googleusercontent.com/a/photo.jpg""#));
        assert!(html.contains(r#"<a href="/logout">Log out</a>"#));
    }

    #[test]
    fn index_renders_login_link_when_not_authenticated() {
        let html = Index {
            user: None,
            client_id: "my_id.apps.googleusercontent.com",
            redirect_uri: "http://localhost:8080/auth/google/callback",
        }
        .render()
        .unwrap();

        assert!(html.contains("Welcome !"));
        assert!(html.contains("client_id=my_id.apps.googleusercontent.com"));
        assert!(!html.contains("/logout"));
    }
}
