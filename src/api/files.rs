/// Files API handlers (WebDAV-backed).

use axum::{
    body::Bytes,
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use crate::state::AppState;
use crate::dav::webdav;

/// GET /api/files
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

    let Some(webdav_cfg) = &state.config.webdav.clone() else {
        return Json(serde_json::json!([])).into_response();
    };

    match webdav::list_files(&webdav_cfg.url, &session.email, &session.password).await {
        Ok(files) => Json(files).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/files  (multipart upload)
pub async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
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

    let Some(webdav_cfg) = &state.config.webdav.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "WebDAV not configured" })),
        )
            .into_response();
    };

    // Extract first file field from the multipart form.
    let field = multipart.next_field().await;
    let Ok(Some(field)) = field else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "no file field" })),
        )
            .into_response();
    };

    let filename = field
        .file_name()
        .unwrap_or("upload")
        .to_string();
    let mime_type = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();
    let data: Bytes = match field.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    match webdav::upload_file(
        &webdav_cfg.url,
        &session.email,
        &session.password,
        &filename,
        &mime_type,
        data,
    )
    .await
    {
        Ok(url) => Json(serde_json::json!({ "url": url })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
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
    Json(req): Json<DeleteRequest>,
) -> impl IntoResponse {
    let session = {
        let inner = state.inner.read().await;
        inner.session.clone()
    };
    let Some(session) = session else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    webdav::delete_file(&req.url, &session.email, &session.password)
        .await
        .ok();

    StatusCode::OK.into_response()
}
