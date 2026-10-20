use crate::AppState;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

/// One session cookie for learners and admins alike — the admin gate is the
/// same session plus a role check, so logging in on either side counts for both.
pub const SESSION_COOKIE: &str = "drill_session";

/// Shortest password accepted at signup and when an admin sets one.
pub const MIN_PASSWORD_LEN: usize = 8;

pub fn hash_password(password: &str) -> String {
    Argon2::default()
        .hash_password(password.as_bytes())
        .expect("password hashing failed")
        .to_string()
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

/// Inserts a 30-day session row and returns the cookie that carries its id.
pub async fn create_session(pool: &PgPool, user_id: Uuid) -> Cookie<'static> {
    let row = sqlx::query(
        "insert into sessions (user_id, expires_at) values ($1, now() + interval '30 days') returning id",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .expect("failed to create session");
    let session_id: Uuid = row.try_get("id").expect("sessions.id");

    crate::db::touch_last_active(pool, user_id).await;

    Cookie::build((SESSION_COOKIE, session_id.to_string()))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .build()
}

/// Deletes the session behind the cookie (if any) and returns the jar with
/// the cookie removed.
pub async fn end_session(pool: &PgPool, jar: CookieJar) -> CookieJar {
    if let Some(session_id) = jar.get(SESSION_COOKIE).and_then(|c| Uuid::parse_str(c.value()).ok()) {
        sqlx::query("delete from sessions where id = $1").bind(session_id).execute(pool).await.ok();
    }
    jar.remove(Cookie::build(SESSION_COOKIE).path("/"))
}

/// Only same-site paths are allowed as a post-login destination, so `next`
/// can't be used to bounce someone to another site.
pub fn safe_next(next: &str) -> String {
    if next.starts_with('/') && !next.starts_with("//") && !next.starts_with("/\\") {
        next.to_string()
    } else {
        "/".to_string()
    }
}

/// A logged-in, active user. Taking this as a handler parameter gates the
/// route: guests are redirected to `/login?next=<this path>`. Use
/// `Option<CurrentUser>` where guests are allowed but the page changes.
pub struct CurrentUser {
    pub id: Uuid,
    pub name: String,
}

pub struct LoginRedirect(String);

impl IntoResponse for LoginRedirect {
    fn into_response(self) -> Response {
        let next: String = form_urlencoded::byte_serialize(self.0.as_bytes()).collect();
        Redirect::to(&format!("/login?next={next}")).into_response()
    }
}

#[axum::async_trait]
impl FromRequestParts<Arc<AppState>> for CurrentUser {
    type Rejection = LoginRedirect;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let redirect = || LoginRedirect(parts.uri.path().to_string());

        let session_id = CookieJar::from_headers(&parts.headers)
            .get(SESSION_COOKIE)
            .and_then(|c| Uuid::parse_str(c.value()).ok())
            .ok_or_else(redirect)?;

        let row = sqlx::query(
            "select u.id, u.name
             from sessions s
             join users u on u.id = s.user_id
             where s.id = $1 and s.expires_at > now() and u.status = 'active'",
        )
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        .expect("session lookup failed")
        .ok_or_else(redirect)?;

        Ok(CurrentUser {
            id: row.try_get("id").expect("users.id"),
            name: row.try_get("name").expect("users.name"),
        })
    }
}
