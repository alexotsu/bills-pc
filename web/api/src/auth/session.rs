use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{Duration as ChronoDuration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::ApiError, models::User, AppState};

pub const SESSION_COOKIE_NAME: &str = "deckgym_session";
const SESSION_TTL_DAYS: i64 = 30;

/// Creates a new session row and returns its id — the id itself is the opaque token stored in
/// the cookie. Revocation (logout, account deletion) is then just deleting the row; there's no
/// JWT-style self-contained token to worry about invalidating separately.
pub async fn create_session(pool: &PgPool, user_id: Uuid) -> Result<Uuid, ApiError> {
    let expires_at = Utc::now() + ChronoDuration::days(SESSION_TTL_DAYS);
    let id: Uuid = sqlx::query_scalar(
        "insert into sessions (user_id, expires_at) values ($1, $2) returning id",
    )
    .bind(user_id)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn delete_session(pool: &PgPool, session_id: Uuid) -> Result<(), ApiError> {
    sqlx::query("delete from sessions where id = $1")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_all_sessions_for_user(pool: &PgPool, user_id: Uuid) -> Result<(), ApiError> {
    sqlx::query("delete from sessions where user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// `secure` should be true in any real deployment (HTTPS); it defaults to false for local dev,
/// where the frontend and API both run over plain http://localhost and a `Secure` cookie would
/// never be sent back. See `AppState::cookie_secure` / the `COOKIE_SECURE` env var.
pub fn build_session_cookie(session_id: Uuid, secure: bool) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE_NAME, session_id.to_string()))
        .http_only(true)
        .secure(secure)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::days(SESSION_TTL_DAYS))
        .build()
}

pub fn build_logout_cookie(secure: bool) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE_NAME, ""))
        .http_only(true)
        .secure(secure)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build()
}

/// Axum extractor for "the user this request's session cookie belongs to." Rejects with 401 if
/// the cookie is missing, malformed, or names a session that's expired or doesn't exist.
pub struct CurrentUser(pub User);

#[axum::async_trait]
impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let session_id = jar
            .get(SESSION_COOKIE_NAME)
            .and_then(|c| Uuid::parse_str(c.value()).ok())
            .ok_or(ApiError::Unauthorized)?;

        let user = sqlx::query_as::<_, User>(
            "select u.* from users u \
             join sessions s on s.user_id = u.id \
             where s.id = $1 and s.expires_at > now()",
        )
        .bind(session_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::Unauthorized)?;

        Ok(CurrentUser(user))
    }
}

/// Like `CurrentUser`, but for routes usable by both logged-in and anonymous callers (e.g.
/// listing decks, which mixes a user's own decks with public reference decks) — `None` instead
/// of a 401 when there's no valid session, and never fails to extract.
pub struct OptionalCurrentUser(pub Option<User>);

#[axum::async_trait]
impl FromRequestParts<AppState> for OptionalCurrentUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match CurrentUser::from_request_parts(parts, state).await {
            Ok(CurrentUser(user)) => Ok(Self(Some(user))),
            Err(_) => Ok(Self(None)),
        }
    }
}
