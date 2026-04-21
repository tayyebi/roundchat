/// Auth API handlers.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::models::Session;

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

/// GET /api/session
pub async fn get_session(State(state): State<AppState>) -> impl IntoResponse {
    let inner = state.inner.read().await;
    match &inner.session {
        Some(s) => Json(SessionResponse {
            email: s.email.clone(),
            logged_in: true,
        })
        .into_response(),
        None => Json(SessionResponse {
            email: String::new(),
            logged_in: false,
        })
        .into_response(),
    }
}

/// POST /api/login
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let email = req.email.trim().to_lowercase();
    let password = req.password.clone();

    // Verify credentials by attempting to fetch via the configured protocol.
    // We only do a lightweight check: connect and authenticate.
    let config = state.config.clone();
    let result = verify_credentials(&config, &email, &password).await;

    match result {
        Ok(()) => {
            let mut inner = state.inner.write().await;
            inner.session = Some(Session {
                email: email.clone(),
                password,
            });
            (
                StatusCode::OK,
                Json(SessionResponse { email, logged_in: true }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/logout
pub async fn logout(State(state): State<AppState>) -> impl IntoResponse {
    let mut inner = state.inner.write().await;
    inner.session = None;
    inner.conversations.clear();
    StatusCode::OK
}

/// Verify credentials by attempting to do a minimal mail-server auth check.
async fn verify_credentials(
    config: &crate::config::AppConfig,
    email: &str,
    password: &str,
) -> anyhow::Result<()> {
    use crate::email::mail_service::fetch_messages_raw;
    // We attempt to fetch (and immediately discard) messages.  If auth fails
    // the underlying library will return an error we surface to the user.
    fetch_messages_raw(config, email, password).await?;
    Ok(())
}
