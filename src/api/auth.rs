/// Authentication API and per-browser session cookies.

use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::email::mail_service;
use crate::models::{MailCache, Session};
use crate::state::AppState;
use crate::storage;

pub const SESSION_COOKIE: &str = "roundchat_session";

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct SessionResponse {
    pub email: String,
    pub logged_in: bool,
}

fn token_from_headers(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|cookie| cookie.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, value)| value)
}

/// Resolve a request to the active account only when its random browser token
/// is still registered. No mailbox credentials are accepted by normal APIs.
pub(crate) async fn authenticated_session(
    state: &AppState,
    headers: &HeaderMap,
) -> Option<Session> {
    let token = token_from_headers(headers)?;
    let inner = state.inner.read().await;
    if !inner.client_tokens.contains(token) {
        return None;
    }
    inner.account.clone()
}

fn logged_out() -> SessionResponse {
    SessionResponse {
        email: String::new(),
        logged_in: false,
    }
}

/// GET /api/session
pub async fn get_session(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    match authenticated_session(&state, &headers).await {
        Some(session) => Json(SessionResponse {
            email: session.email,
            logged_in: true,
        }),
        None => Json(logged_out()),
    }
}

/// POST /api/login
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Response {
    let email = request.email.trim().to_lowercase();
    if email.is_empty() || request.password.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "email and password are required" })),
        )
            .into_response();
    }

    let _guard = state.sync_lock.lock().await;
    {
        let inner = state.inner.read().await;
        if let Some(account) = &inner.account {
            if account.email != email {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "error": "another mailbox is active; sign it out before switching accounts"
                    })),
                )
                    .into_response();
            }
        }
    }

    if let Err(error) = mail_service::verify_credentials(&state.config, &email, &request.password).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": error.to_string() })),
        )
            .into_response();
    }

    let existing_cache = state.inner.read().await.cache.clone();
    let cache_path = storage::cache_path(&state.config.data_dir, &email);
    let cache = if !existing_cache.messages.is_empty() {
        existing_cache
    } else if cache_path.exists() {
        storage::load_mail_cache(&state.config.data_dir, &email).unwrap_or_else(|error| {
            tracing::warn!("Could not restore chat cache: {error}");
            MailCache::default()
        })
    } else {
        MailCache::default()
    };
    let conversations = crate::email::conversation::build_conversations(
        cache.messages.clone(),
        &email,
    );
    let token = uuid::Uuid::new_v4().to_string();
    {
        let mut inner = state.inner.write().await;
        inner.account = Some(Session {
            email: email.clone(),
            password: request.password,
        });
        inner.client_tokens.insert(token.clone());
        inner.cache = cache;
        inner.conversations = conversations;
    }

    let cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=86400"
    );
    let mut response = (
        StatusCode::OK,
        Json(SessionResponse {
            email,
            logged_in: true,
        }),
    )
        .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("generated session cookie is valid"),
    );
    response
}

/// POST /api/logout
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = token_from_headers(&headers) {
        let mut inner = state.inner.write().await;
        inner.client_tokens.remove(token);
        if inner.client_tokens.is_empty() {
            inner.account = None;
            inner.conversations.clear();
            inner.cache = MailCache::default();
        }
    }

    let mut response = StatusCode::OK.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static(
            "roundchat_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
        ),
    );
    response
}
