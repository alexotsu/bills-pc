use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::Deserialize;

use crate::config::OAuthConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Google,
    Facebook,
}

impl Provider {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "google" => Some(Self::Google),
            "facebook" => Some(Self::Facebook),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Facebook => "facebook",
        }
    }
}

/// Builds an OAuth2 client for `provider`. Errors if that provider's client id/secret aren't
/// configured (see `web/README.md` / the env vars documented in `Config`) — this is checked at
/// request time rather than at server boot, so the server still starts fine before OAuth
/// credentials exist; only actually starting that provider's flow fails until they're set.
pub fn build_client(provider: Provider, config: &OAuthConfig) -> anyhow::Result<BasicClient> {
    let (auth_url, token_url, client_id, client_secret) = match provider {
        Provider::Google => (
            "https://accounts.google.com/o/oauth2/v2/auth",
            "https://oauth2.googleapis.com/token",
            &config.google_client_id,
            &config.google_client_secret,
        ),
        Provider::Facebook => (
            "https://www.facebook.com/v19.0/dialog/oauth",
            "https://graph.facebook.com/v19.0/oauth/access_token",
            &config.facebook_client_id,
            &config.facebook_client_secret,
        ),
    };

    if client_id.is_empty() || client_secret.is_empty() {
        anyhow::bail!(
            "{} OAuth is not configured (missing client id/secret)",
            provider.as_str()
        );
    }

    let redirect_url = format!(
        "{}/api/auth/oauth/{}/callback",
        config.api_base_url,
        provider.as_str()
    );

    Ok(BasicClient::new(
        ClientId::new(client_id.clone()),
        Some(ClientSecret::new(client_secret.clone())),
        AuthUrl::new(auth_url.to_string())?,
        Some(TokenUrl::new(token_url.to_string())?),
    )
    .set_redirect_uri(RedirectUrl::new(redirect_url)?))
}

pub struct AuthorizationRequest {
    pub url: String,
    pub csrf_token: String,
    pub pkce_verifier: String,
}

pub fn authorization_request(client: &BasicClient, provider: Provider) -> AuthorizationRequest {
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let mut request = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()));
    if provider == Provider::Google {
        // Required for the openidconnect.googleapis.com/v1/userinfo endpoint
        // (exchange_and_fetch_profile) to return `sub` at all — without it Google's token is
        // scoped for the legacy, non-OIDC userinfo endpoint instead.
        request = request.add_scope(Scope::new("openid".to_string()));
    }
    let (auth_url, csrf_token) = request.set_pkce_challenge(pkce_challenge).url();

    AuthorizationRequest {
        url: auth_url.to_string(),
        csrf_token: csrf_token.secret().clone(),
        pkce_verifier: pkce_verifier.secret().clone(),
    }
}

pub struct OAuthProfile {
    pub subject: String,
    pub email: Option<String>,
}

#[derive(Deserialize)]
struct GoogleUserInfo {
    sub: String,
    email: Option<String>,
}

#[derive(Deserialize)]
struct FacebookUserInfo {
    id: String,
    email: Option<String>,
}

/// Exchanges `code` for an access token, then fetches the provider's profile endpoint to learn
/// the user's stable subject id (and email, if they've granted that scope).
pub async fn exchange_and_fetch_profile(
    http: &reqwest::Client,
    client: &BasicClient,
    code: String,
    pkce_verifier: String,
    provider: Provider,
) -> anyhow::Result<OAuthProfile> {
    let token = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
        .request_async(oauth2::reqwest::async_http_client)
        .await
        .map_err(|e| anyhow::anyhow!("token exchange failed: {e}"))?;

    let access_token = token.access_token().secret();

    match provider {
        Provider::Google => {
            let info: GoogleUserInfo = http
                .get("https://openidconnect.googleapis.com/v1/userinfo")
                .bearer_auth(access_token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            Ok(OAuthProfile {
                subject: info.sub,
                email: info.email,
            })
        }
        Provider::Facebook => {
            let info: FacebookUserInfo = http
                .get("https://graph.facebook.com/me?fields=id,email")
                .bearer_auth(access_token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            Ok(OAuthProfile {
                subject: info.id,
                email: info.email,
            })
        }
    }
}
