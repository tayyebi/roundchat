/// Contacts API handler.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use crate::state::AppState;
use crate::dav::carddav;

/// GET /api/contacts
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

    let Some(carddav_cfg) = &state.config.carddav.clone() else {
        return Json(serde_json::json!([])).into_response();
    };

    match carddav::fetch_all_contacts(
        &carddav_cfg.url,
        &session.email,
        &session.password,
    )
    .await
    {
        Ok(contacts) => Json(contacts).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
