/// Contacts API handler.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::api::auth::authenticated_session;
use crate::dav::carddav;
use crate::state::AppState;

/// GET /api/contacts
pub async fn list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(session) = authenticated_session(&state, &headers).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "not logged in" })),
        )
            .into_response();
    };

    let Some(carddav_config) = &state.config.carddav else {
        return Json(serde_json::json!([])).into_response();
    };

    match carddav::fetch_all_contacts(
        &carddav_config.url,
        &session.email,
        &session.password,
    )
    .await
    {
        Ok(contacts) => Json(contacts).into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}
