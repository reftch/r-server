use askama::Template;
use r_server::{
    client::Client,
    core::http::Server,
    request::Request,
    response::{ContentType, Response, Status},
    router::Method,
};
use serde::{Deserialize, Serialize};

const DEFAULT_REDIRECT_URI: &str = "http://localhost:8080/auth/keycloak/callback";
const USER_SESSION_KEY: &str = "keycloak_user";

fn issuer() -> String {
    std::env::var("KEYCLOAK_URL")
        .expect("KEYCLOAK_URL must be set in the environment or a .env file")
}

/// Splits the issuer URL into a host accepted by `Client::new` (scheme + authority,
/// default port only) and the realm path prefix used to build OIDC endpoints.
///
/// `Client` only supports the default ports (80/443) and has no notion of a path,
/// so `http://localhost/realms/rserver` becomes `("http://localhost", "/realms/rserver")`.
fn issuer_parts() -> (String, String) {
    let url = issuer();
    let (rest, scheme) = if let Some(r) = url.strip_prefix("https://") {
        (r, "https://")
    } else if let Some(r) = url.strip_prefix("http://") {
        (r, "http://")
    } else {
        (url.as_str(), "http://")
    };

    let slash = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..slash];
    let path = &rest[slash..];
    (format!("{}{}", scheme, authority), path.to_string())
}

fn client_id() -> String {
    std::env::var("KEYCLOAK_CLIENT_ID")
        .expect("KEYCLOAK_CLIENT_ID must be set in the environment or a .env file")
}

fn redirect_uri() -> String {
    std::env::var("KEYCLOAK_CALLBACK").unwrap_or_else(|_| DEFAULT_REDIRECT_URI.to_string())
}

#[derive(Template)]
#[template(path = "index.html")]
struct Index<'a> {
    user: Option<UserView<'a>>,
    authorize_url: String,
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
struct KeycloakUser {
    sub: String,
    name: Option<String>,
    email: Option<String>,
    preferred_username: Option<String>,
    picture: Option<String>,
}

fn current_user(req: &Request) -> Option<KeycloakUser> {
    let raw = req.session()?.get(USER_SESSION_KEY)?;
    serde_json::from_str(&raw).ok()
}

fn index(req: &Request, res: &mut Response) {
    let kc_user = current_user(req);
    let user = kc_user.as_ref().map(|u| UserView {
        name: u
            .name
            .as_deref()
            .or(u.preferred_username.as_deref())
            .unwrap_or(&u.sub),
        picture: u.picture.as_deref(),
    });

    let authorize_url = format!(
        "{}/protocol/openid-connect/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid",
        issuer(),
        client_id(),
        redirect_uri()
    );

    let html = Index {
        user,
        authorize_url,
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
        std::env::var("KEYCLOAK_CLIENT_SECRET").unwrap_or_default(),
        redirect_uri()
    );

    let (kc_host, realm_path) = issuer_parts();
    let kc = Client::new(&kc_host);

    let token_path = format!("{}/protocol/openid-connect/token", realm_path);
    let userinfo_path = format!("{}/protocol/openid-connect/userinfo", realm_path);

    let token_resp_body = match kc.post_with(
        &token_path,
        token_body,
        &[("Content-Type", "application/x-www-form-urlencoded")],
    ) {
        Ok(body) => body,
        Err(_) => {
            res.status(Status::InternalServerError)
                .body("Failed to communicate with Keycloak");
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

    let user_body = match kc.get_with(
        &userinfo_path,
        &[(
            "Authorization",
            &format!("Bearer {}", token_data.access_token),
        )],
    ) {
        Ok(body) => body,
        Err(_) => {
            res.status(Status::InternalServerError)
                .body("Failed to fetch user profile from Keycloak");
            return;
        }
    };

    let kc_user: KeycloakUser = match serde_json::from_str(&user_body) {
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

    let Ok(profile_json) = serde_json::to_string(&kc_user) else {
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
        .route(Method::GET, "/auth/keycloak/callback", callback)
        .route(Method::GET, "/logout", logout)
        .assets_path(concat!(env!("CARGO_MANIFEST_DIR"), "/templates"))
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_user() -> KeycloakUser {
        KeycloakUser {
            sub: "a1b2c3d4-1234-5678-9abc-def012345678".into(),
            name: Some("Ada Lovelace".into()),
            email: Some("ada@example.com".into()),
            preferred_username: Some("ada".into()),
            picture: Some("https://example.com/avatar.jpg".into()),
        }
    }

    #[test]
    fn user_survives_session_roundtrip() {
        let raw = serde_json::to_string(&sample_user()).unwrap();
        let restored: KeycloakUser = serde_json::from_str(&raw).unwrap();

        assert_eq!(restored.sub, "a1b2c3d4-1234-5678-9abc-def012345678");
        assert_eq!(restored.name.as_deref(), Some("Ada Lovelace"));
        assert_eq!(restored.email.as_deref(), Some("ada@example.com"));
        assert_eq!(restored.preferred_username.as_deref(), Some("ada"));
        assert_eq!(
            restored.picture.as_deref(),
            Some("https://example.com/avatar.jpg")
        );
    }

    #[test]
    fn index_renders_logged_in_user_from_session_data() {
        let u = sample_user();
        let html = Index {
            user: Some(UserView {
                name: u
                    .name
                    .as_deref()
                    .or(u.preferred_username.as_deref())
                    .unwrap_or(&u.sub),
                picture: u.picture.as_deref(),
            }),
            authorize_url: "http://localhost:8080/realms/master/protocol/openid-connect/auth"
                .into(),
        }
        .render()
        .unwrap();

        assert!(html.contains("Hi, Ada Lovelace!"));
        assert!(html.contains(r#"src="https://example.com/avatar.jpg""#));
        assert!(html.contains(r#"<a href="/logout""#));
        assert!(html.contains("Log out</a>"));
    }

    #[test]
    fn index_renders_login_link_when_not_authenticated() {
        let html = Index {
            user: None,
            authorize_url: "http://localhost:8080/realms/master/protocol/openid-connect/auth?client_id=kc_id&redirect_uri=http://localhost:8080/auth/keycloak/callback&response_type=code&scope=openid".into(),
        }
        .render()
        .unwrap();

        assert!(html.contains("Welcome !"));
        assert!(html.contains("protocol/openid-connect/auth"));
        assert!(html.contains("client_id=kc_id"));
        assert!(!html.contains("Log out</a>"));
    }
}
