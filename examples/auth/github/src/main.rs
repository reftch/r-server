use askama::Template;
use r_server::{
    client::Client,
    core::http::Server,
    request::Request,
    response::{ContentType, Response, Status},
    router::Method,
};
use serde::{Deserialize, Serialize};

const DEFAULT_REDIRECT_URI: &str = "http://localhost:8080/auth/github/callback";
const USER_SESSION_KEY: &str = "github_user";

fn client_id() -> String {
    std::env::var("GITHUB_KEY").expect("GITHUB_KEY must be set in the environment or a .env file")
}

fn redirect_uri() -> String {
    std::env::var("GITHUB_CALLBACK").unwrap_or_else(|_| DEFAULT_REDIRECT_URI.to_string())
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
    avatar: Option<&'a str>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Serialize, Deserialize)]
struct GitHubUser {
    login: String,
    name: Option<String>,
    avatar_url: Option<String>,
}

fn current_user(req: &Request) -> Option<GitHubUser> {
    let raw = req.session()?.get(USER_SESSION_KEY)?;
    serde_json::from_str(&raw).ok()
}

fn index(req: &Request, res: &mut Response) {
    let gh_user = current_user(req);
    let user = gh_user.as_ref().map(|u| UserView {
        name: u.name.as_deref().unwrap_or(&u.login),
        avatar: u.avatar_url.as_deref(),
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

    let github = Client::new("https://github.com");
    let token_path = format!(
        "/login/oauth/access_token?client_id={}&client_secret={}&code={}",
        client_id(),
        std::env::var("GITHUB_SECRET").unwrap_or_default(),
        code
    );

    let token_body = match github.post(&token_path, String::new()) {
        Ok(body) => body,
        Err(_) => {
            res.status(Status::InternalServerError)
                .body("Failed to communicate with GitHub");
            return;
        }
    };

    let token_data: TokenResponse = match serde_json::from_str(&token_body) {
        Ok(data) => data,
        Err(_) => {
            res.status(Status::BadRequest)
                .body("Invalid or expired authorization code");
            return;
        }
    };

    let api = Client::new("https://api.github.com");
    let user_body = match api.get_with(
        "/user",
        &[(
            "Authorization",
            &format!("Bearer {}", token_data.access_token),
        )],
    ) {
        Ok(body) => body,
        Err(_) => {
            res.status(Status::InternalServerError)
                .body("Failed to fetch user profile from GitHub");
            return;
        }
    };

    let gh_user: GitHubUser = match serde_json::from_str(&user_body) {
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

    let Ok(profile_json) = serde_json::to_string(&gh_user) else {
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
        .route(Method::GET, "/auth/github/callback", callback)
        .route(Method::GET, "/logout", logout)
        .assets_path(concat!(env!("CARGO_MANIFEST_DIR"), "/templates"))
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_user() -> GitHubUser {
        GitHubUser {
            login: "octocat".into(),
            name: Some("The Octocat".into()),
            avatar_url: Some("https://avatars.githubusercontent.com/u/583231".into()),
        }
    }

    #[test]
    fn user_survives_session_roundtrip() {
        let raw = serde_json::to_string(&sample_user()).unwrap();
        let restored: GitHubUser = serde_json::from_str(&raw).unwrap();

        assert_eq!(restored.login, "octocat");
        assert_eq!(restored.name.as_deref(), Some("The Octocat"));
        assert_eq!(
            restored.avatar_url.as_deref(),
            Some("https://avatars.githubusercontent.com/u/583231")
        );
    }

    #[test]
    fn index_renders_logged_in_user_from_session_data() {
        let u = sample_user();
        let html = Index {
            user: Some(UserView {
                name: u.name.as_deref().unwrap_or(&u.login),
                avatar: u.avatar_url.as_deref(),
            }),
            client_id: "k",
            redirect_uri: "r",
        }
        .render()
        .unwrap();

        assert!(html.contains("Hi, The Octocat!"));
        assert!(html.contains(r#"src="https://avatars.githubusercontent.com/u/583231""#));
        assert!(html.contains(r#"<a href="/logout">Log out</a>"#));
    }

    #[test]
    fn index_renders_login_link_when_not_authenticated() {
        let html = Index {
            user: None,
            client_id: "my_id",
            redirect_uri: "http://localhost:8080/auth/github/callback",
        }
        .render()
        .unwrap();

        assert!(html.contains("Welcome !"));
        assert!(html.contains("client_id=my_id"));
        assert!(!html.contains("/logout"));
    }
}
