pub mod auth;
pub mod contacts;
pub mod conversations;
pub mod files;
pub mod sse;

use axum::{
    routing::{delete, get, post},
    Router,
};
use crate::state::AppState;

/// Build the full axum router.
pub fn router(state: AppState) -> Router {
    Router::new()
        // Static frontend files (embedded in binary).
        .route("/", get(serve_index))
        .route("/style.css", get(serve_css))
        .route("/app.js", get(serve_js))
        // Auth
        .route("/api/session", get(auth::get_session))
        .route("/api/login", post(auth::login))
        .route("/api/logout", post(auth::logout))
        // Conversations
        .route("/api/conversations", get(conversations::list))
        .route("/api/send", post(conversations::send))
        .route("/api/mark-read/:uid", post(conversations::mark_read))
        // Contacts
        .route("/api/contacts", get(contacts::list))
        // Files
        .route("/api/files", get(files::list))
        .route("/api/files", post(files::upload))
        .route("/api/files", delete(files::remove))
        // SSE
        .route("/api/events", get(sse::handler))
        .with_state(state)
}

// Embedded frontend assets.
const INDEX_HTML: &str = include_str!("../../frontend/index.html");
const STYLE_CSS: &str = include_str!("../../frontend/style.css");
const APP_JS: &str = include_str!("../../frontend/app.js");

async fn serve_index() -> axum::response::Html<&'static str> {
    axum::response::Html(INDEX_HTML)
}

async fn serve_css() -> (axum::http::StatusCode, [(axum::http::HeaderName, &'static str); 1], &'static str) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        STYLE_CSS,
    )
}

async fn serve_js() -> (axum::http::StatusCode, [(axum::http::HeaderName, &'static str); 1], &'static str) {
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        APP_JS,
    )
}
