/// OAuth client credentials, kept separate from the rest of `Config` since they're passed as a
/// unit into `auth::oauth::build_client`.
#[derive(Clone, Default)]
pub struct OAuthConfig {
    pub google_client_id: String,
    pub google_client_secret: String,
    pub facebook_client_id: String,
    pub facebook_client_secret: String,
    /// Must exactly match this server's public base URL — it's used to build the OAuth
    /// redirect URI (`{api_base_url}/api/auth/oauth/:provider/callback`), which must in turn
    /// exactly match what's registered with each provider.
    pub api_base_url: String,
}

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub oauth: OAuthConfig,
    /// Where to send the browser after a successful OAuth login.
    pub frontend_url: String,
    /// Whether session/oauth-state cookies get the `Secure` attribute. Must be true for any
    /// real (HTTPS) deployment; defaults to false so local http://localhost dev works without
    /// extra setup — a `Secure` cookie is never sent back over plain HTTP.
    pub cookie_secure: bool,
    /// Base URL of the external host serving card art (e.g. an S3/R2 bucket or image CDN) —
    /// deliberately not stored in this repo (see `cards.rs`'s `image_url`): official TCG art is
    /// under a license this repo doesn't hold, and committed image files would also bloat the
    /// git history. `None` until a host is chosen, in which case `image_url` stays `None` for
    /// every card and the frontend falls back to its plain name box.
    pub card_image_base_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set (see web/README.md)"),
            oauth: OAuthConfig {
                google_client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
                google_client_secret: std::env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
                facebook_client_id: std::env::var("FACEBOOK_CLIENT_ID").unwrap_or_default(),
                facebook_client_secret: std::env::var("FACEBOOK_CLIENT_SECRET").unwrap_or_default(),
                api_base_url: std::env::var("API_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            },
            frontend_url: std::env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            cookie_secure: std::env::var("COOKIE_SECURE")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            card_image_base_url: std::env::var("CARD_IMAGE_BASE_URL").ok(),
        }
    }
}
