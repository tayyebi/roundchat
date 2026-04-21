/// Conversation API handlers.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use crate::state::AppState;
use crate::email::mail_service;

/// GET /api/conversations
pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    let session = {
        let inner = state.inner.read().await;
        inner.session.clone()
    };
    let Some(session) = session else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "not logged in" })),
        )
            .into_response();
    };

    match mail_service::fetch_conversations(
        &state.config,
        &session.email,
        &session.password,
    )
    .await
    {
        Ok(convos) => {
            // Cache the result.
            let mut inner = state.inner.write().await;
            inner.conversations = convos.clone();
            // Notify SSE subscribers.
            state.events.send(crate::state::AppEvent::ConversationsUpdated).ok();
            Json(convos).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct SendRequest {
    pub to: Vec<String>,
    pub body: String,
    pub group_name: Option<String>,
}

/// POST /api/send
pub async fn send(
    State(state): State<AppState>,
    Json(req): Json<SendRequest>,
) -> impl IntoResponse {
    let session = {
        let inner = state.inner.read().await;
        inner.session.clone()
    };
    let Some(session) = session else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "not logged in" })),
        )
            .into_response();
    };

    match mail_service::send_message(
        &state.config,
        &session.email,
        &session.password,
        req.to,
        req.body,
        req.group_name,
    )
    .await
    {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/mark-read/:uid
pub async fn mark_read(
    State(state): State<AppState>,
    Path(uid): Path<String>,
) -> impl IntoResponse {
    let session = {
        let inner = state.inner.read().await;
        inner.session.clone()
    };
    let Some(session) = session else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    mail_service::mark_read(&state.config, &session.email, &session.password, &uid)
        .await
        .ok();

    StatusCode::OK.into_response()
}
