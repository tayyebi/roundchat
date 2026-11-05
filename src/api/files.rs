/// Files API handlers (WebDAV-backed).

use axum::{
    body::Bytes,
    extract::{Multipart, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::api::auth::authenticated_session;
use crate::dav::webdav;
use crate::state::AppState;

/// GET /api/files
pub async fn list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(session) = authenticated_session(&state, &headers).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "not logged in" })),
        )
            .into_response();
    };
    let Some(webdav_config) = &state.config.webdav else {
        return Json(serde_json::json!([])).into_response();
    };

    match webdav::list_files(&webdav_config.url, &session.email, &session.password).await {
        Ok(files) => Json(files).into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/files (multipart upload)
pub async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let Some(session) = authenticated_session(&state, &headers).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "not logged in" })),
        )
            .into_response();
    };
    let Some(webdav_config) = &state.config.webdav else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "WebDAV not configured" })),
        )
            .into_response();
    };

    let field = multipart.next_field().await;
    let Ok(Some(field)) = field else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "no file field" })),
        )
            .into_response();
    };
    let filename = field.file_name().unwrap_or("upload").to_string();
    let mime_type = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();
    let data: Bytes = match field.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": error.to_string() })),
            )
                .into_response();
        }
    };

    match webdav::upload_file(
        &webdav_config.url,
        &session.email,
        &session.password,
        &filename,
        &mime_type,
        data,
    )
    .await
    {
        Ok(url) => Json(serde_json::json!({ "url": url })).into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct DeleteRequest {
    pub url: String,
}

/// DELETE /api/files
pub async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeleteRequest>,
) -> Response {
    let Some(session) = authenticated_session(&state, &headers).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(webdav_config) = &state.config.webdav else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    match webdav::delete_file(
        &webdav_config.url,
        &request.url,
        &session.email,
        &session.password,
    )
    .await
    {
        Ok(()) => StatusCode::OK.into_response(),
        Err(error) => {
            tracing::warn!("WebDAV delete failed: {error}");
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": error.to_string() })),
            )
                .into_response()
        }
    }
}
