/// Server-Sent Events handler.

use std::convert::Infallible;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::api::auth::authenticated_session;
use crate::state::{AppEvent, AppState};

/// GET /api/events
pub async fn handler(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if authenticated_session(&state, &headers).await.is_none() {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let receiver = state.events.subscribe();
    let stream = BroadcastStream::new(receiver).filter_map(|message| match message {
        Ok(AppEvent::ConversationsUpdated) => Some(Ok::<Event, Infallible>(
            Event::default().event("refresh").data(""),
        )),
        Err(_) => {
            tracing::debug!("SSE subscriber lagged behind, dropping event");
            None
        }
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}
