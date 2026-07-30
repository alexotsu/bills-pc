use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
    Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, models::User, AppState};

use super::{
    oauth::{self, Provider},
    password, session,
};

const OAUTH_STATE_COOKIE: &str = "deckgym_oauth_state";
const PENDING_OAUTH_COOKIE: &str = "deckgym_pending_oauth";
const MIN_PASSWORD_LEN: usize = 8;

// ---------------------------------------------------------------------------------------------
// Email + password
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct RegisterRequest {
    email: String,
    password: String,
    /// Required and must be `true` — see `web/SPEC.md`: "no opt-in, no account." Enforced here
    /// for every signup path (this one and `complete_oauth_signup` below), not just this one.
    training_data_opt_in: bool,
}

pub async fn register(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(req): Json<RegisterRequest>,
) -> Result<(CookieJar, Json<User>), ApiError> {
    if !req.training_data_opt_in {
        return Err(ApiError::BadRequest(
            "training_data_opt_in must be true to create an account".to_string(),
        ));
    }
    let email = req.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(ApiError::BadRequest(
            "a valid email is required".to_string(),
        ));
    }
    if req.password.len() < MIN_PASSWORD_LEN {
        return Err(ApiError::BadRequest(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }

    let password_hash = password::hash_password(&req.password).map_err(ApiError::Internal)?;

    let user = sqlx::query_as::<_, User>(
        "insert into users (email, password_hash, training_data_opt_in) \
         values ($1, $2, true) returning *",
    )
    .bind(&email)
    .bind(&password_hash)
    .fetch_one(&state.db)
    .await
    .map_err(map_unique_violation)?;

    let session_id = session::create_session(&state.db, user.id).await?;
    let cookie = session::build_session_cookie(session_id, state.config.cookie_secure);
    Ok((jar.add(cookie), Json(user)))
}

#[derive(Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> Result<(CookieJar, Json<User>), ApiError> {
    let email = req.email.trim().to_lowercase();
    let user = sqlx::query_as::<_, User>("select * from users where email = $1")
        .bind(&email)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    // No password hash means this is an OAuth-only account (or one whose PII was deleted) —
    // either way, password login can't succeed for it. Don't distinguish this case in the
    // response from "wrong password," to avoid leaking account existence/type.
    let matches = user
        .password_hash
        .as_deref()
        .is_some_and(|hash| password::verify_password(&req.password, hash));
    if !matches {
        return Err(ApiError::Unauthorized);
    }

    let session_id = session::create_session(&state.db, user.id).await?;
    let cookie = session::build_session_cookie(session_id, state.config.cookie_secure);
    Ok((jar.add(cookie), Json(user)))
}

pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), ApiError> {
    if let Some(cookie) = jar.get(session::SESSION_COOKIE_NAME) {
        if let Ok(session_id) = Uuid::parse_str(cookie.value()) {
            session::delete_session(&state.db, session_id).await?;
        }
    }
    let jar = jar.add(session::build_logout_cookie(state.config.cookie_secure));
    Ok((jar, StatusCode::NO_CONTENT))
}

pub async fn me(session::CurrentUser(user): session::CurrentUser) -> Json<User> {
    Json(user)
}

/// GDPR deletion: strips PII (email, password hash, OAuth identity) from the user's row and
/// revokes all their sessions, but the row itself and all their `decks`/`games`/`game_plies`
/// stay intact — per `SPEC.md`, deletion removes PII, never training data (consent for that was
/// already given, irrevocably, at signup).
pub async fn delete_account(
    State(state): State<AppState>,
    session::CurrentUser(user): session::CurrentUser,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), ApiError> {
    sqlx::query(
        "update users set email = null, password_hash = null, \
         oauth_provider = null, oauth_subject = null where id = $1",
    )
    .bind(user.id)
    .execute(&state.db)
    .await?;
    session::delete_all_sessions_for_user(&state.db, user.id).await?;

    let jar = jar.add(session::build_logout_cookie(state.config.cookie_secure));
    Ok((jar, StatusCode::NO_CONTENT))
}

fn map_unique_violation(e: sqlx::Error) -> ApiError {
    match &e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            ApiError::Conflict("an account with that identity already exists".to_string())
        }
        _ => ApiError::Database(e),
    }
}

// ---------------------------------------------------------------------------------------------
// OAuth (Google, Facebook)
// ---------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct StoredOAuthState {
    csrf: String,
    verifier: String,
    provider: String,
}

pub async fn oauth_start(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    jar: CookieJar,
) -> Result<impl IntoResponse, ApiError> {
    let provider = parse_provider(&provider)?;
    let client = oauth::build_client(provider, &state.config.oauth).map_err(ApiError::Internal)?;
    let req = oauth::authorization_request(&client, provider);

    let stored = StoredOAuthState {
        csrf: req.csrf_token,
        verifier: req.pkce_verifier,
        provider: provider.as_str().to_string(),
    };
    let payload = serde_json::to_string(&stored).map_err(|e| ApiError::Internal(e.into()))?;

    let cookie = Cookie::build((OAUTH_STATE_COOKIE, payload))
        .http_only(true)
        .secure(state.config.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/api/auth")
        .max_age(time::Duration::minutes(10))
        .build();

    Ok((jar.add(cookie), Redirect::to(&req.url)))
}

#[derive(Deserialize)]
pub struct OAuthCallbackQuery {
    code: String,
    state: String,
}

/// On a *returning* identity, logs them in directly. On a *first-time* identity, doesn't create
/// the account yet — stashes the verified provider identity in a short-lived cookie and sends
/// the browser to the frontend to explicitly confirm the training-data opt-in first (see
/// `complete_oauth_signup`), so "no opt-in, no account" holds for every signup path, not just
/// email/password.
pub async fn oauth_callback(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(query): Query<OAuthCallbackQuery>,
    jar: CookieJar,
) -> Result<impl IntoResponse, ApiError> {
    let provider = parse_provider(&provider)?;

    let raw = jar.get(OAUTH_STATE_COOKIE).ok_or_else(|| {
        ApiError::BadRequest("missing oauth state; please try signing in again".to_string())
    })?;
    let stored: StoredOAuthState = serde_json::from_str(raw.value())
        .map_err(|_| ApiError::BadRequest("invalid oauth state; please try again".to_string()))?;
    if stored.csrf != query.state || stored.provider != provider.as_str() {
        return Err(ApiError::BadRequest(
            "oauth state mismatch; please try again".to_string(),
        ));
    }

    let client = oauth::build_client(provider, &state.config.oauth).map_err(ApiError::Internal)?;
    let profile = oauth::exchange_and_fetch_profile(
        &state.http,
        &client,
        query.code,
        stored.verifier,
        provider,
    )
    .await
    .map_err(ApiError::Internal)?;

    let jar = jar.remove(Cookie::from(OAUTH_STATE_COOKIE));

    let existing = sqlx::query_as::<_, User>(
        "select * from users where oauth_provider = $1 and oauth_subject = $2",
    )
    .bind(provider.as_str())
    .bind(&profile.subject)
    .fetch_optional(&state.db)
    .await?;

    if let Some(user) = existing {
        let session_id = session::create_session(&state.db, user.id).await?;
        let jar = jar.add(session::build_session_cookie(
            session_id,
            state.config.cookie_secure,
        ));
        return Ok((jar, Redirect::to(&state.config.frontend_url)));
    }

    let pending = PendingOAuthSignup {
        provider: provider.as_str().to_string(),
        subject: profile.subject,
        email: profile.email,
    };
    let payload = serde_json::to_string(&pending).map_err(|e| ApiError::Internal(e.into()))?;
    let pending_cookie = Cookie::build((PENDING_OAUTH_COOKIE, payload))
        .http_only(true)
        .secure(state.config.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/api/auth")
        .max_age(time::Duration::minutes(10))
        .build();

    let redirect_url = format!(
        "{}/complete-signup?provider={}",
        state.config.frontend_url,
        provider.as_str()
    );
    Ok((jar.add(pending_cookie), Redirect::to(&redirect_url)))
}

#[derive(Serialize, Deserialize)]
struct PendingOAuthSignup {
    provider: String,
    subject: String,
    email: Option<String>,
}

#[derive(Deserialize)]
pub struct CompleteOAuthSignupRequest {
    training_data_opt_in: bool,
}

pub async fn complete_oauth_signup(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(req): Json<CompleteOAuthSignupRequest>,
) -> Result<(CookieJar, Json<User>), ApiError> {
    if !req.training_data_opt_in {
        return Err(ApiError::BadRequest(
            "training_data_opt_in must be true to create an account".to_string(),
        ));
    }

    let raw = jar.get(PENDING_OAUTH_COOKIE).ok_or_else(|| {
        ApiError::BadRequest("no pending sign-in found; please start over".to_string())
    })?;
    let pending: PendingOAuthSignup = serde_json::from_str(raw.value())
        .map_err(|_| ApiError::BadRequest("invalid pending sign-in".to_string()))?;

    let user = sqlx::query_as::<_, User>(
        "insert into users (email, oauth_provider, oauth_subject, training_data_opt_in) \
         values ($1, $2, $3, true) returning *",
    )
    .bind(&pending.email)
    .bind(&pending.provider)
    .bind(&pending.subject)
    .fetch_one(&state.db)
    .await
    .map_err(map_unique_violation)?;

    let session_id = session::create_session(&state.db, user.id).await?;
    let jar = jar
        .remove(Cookie::from(PENDING_OAUTH_COOKIE))
        .add(session::build_session_cookie(
            session_id,
            state.config.cookie_secure,
        ));
    Ok((jar, Json(user)))
}

fn parse_provider(s: &str) -> Result<Provider, ApiError> {
    Provider::parse(s).ok_or_else(|| ApiError::BadRequest(format!("unknown oauth provider: {s}")))
}
