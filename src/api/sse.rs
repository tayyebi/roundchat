/// Server-Sent Events handler.
///
/// Clients subscribe to GET /api/events and receive a "refresh" event whenever
/// the conversation list changes (triggered by the background polling task or
/// by a manual fetch).

use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
};
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use crate::state::{AppEvent, AppState};

/// GET /api/events
pub async fn handler(State(state): State<AppState>) -> impl IntoResponse {
    let rx = state.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| {
        match msg {
            Ok(AppEvent::ConversationsUpdated) => {
                Some(Ok::<Event, Infallible>(
                    Event::default().event("refresh").data(""),
                ))
            }
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
