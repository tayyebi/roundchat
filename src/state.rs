/// Shared application state threaded through axum handlers.

use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use crate::config::AppConfig;
use crate::models::{Conversation, Session};

/// Events pushed to SSE subscribers when the conversation list changes.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// The full conversation list was refreshed; clients should re-fetch.
    ConversationsUpdated,
}

/// Inner state protected by an RwLock.
#[derive(Debug, Default)]
pub struct Inner {
    pub session: Option<Session>,
    /// Cached conversations, updated by the background polling task.
    pub conversations: Vec<Conversation>,
}

/// Arc-wrapped state shared across all axum route handlers.
#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<RwLock<Inner>>,
    pub config: Arc<AppConfig>,
    /// Broadcast channel for pushing refresh events to SSE streams.
    pub events: broadcast::Sender<AppEvent>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let (tx, _) = broadcast::channel(64);
        AppState {
            inner: Arc::new(RwLock::new(Inner::default())),
            config: Arc::new(config),
            events: tx,
        }
    }
}
