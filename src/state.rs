/// Shared application state threaded through axum handlers.

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::{broadcast, Mutex, RwLock};

use crate::config::AppConfig;
use crate::models::{Conversation, MailCache, Session};

/// Events pushed to SSE subscribers when the conversation list changes.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// The full conversation list was refreshed; clients should re-fetch.
    ConversationsUpdated,
}

/// Inner state protected by an RwLock.
#[derive(Debug, Default)]
pub struct Inner {
    /// RoundChat remains a single-mailbox application, but each browser must
    /// present its own random token before it can use this account.
    pub account: Option<Session>,
    pub client_tokens: HashSet<String>,
    pub conversations: Vec<Conversation>,
    pub cache: MailCache,
}

/// Arc-wrapped state shared across all axum route handlers.
#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<RwLock<Inner>>,
    pub config: Arc<AppConfig>,
    /// Serializes network sync and cache mutation for the active account.
    pub sync_lock: Arc<Mutex<()>>,
    pub events: broadcast::Sender<AppEvent>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let (tx, _) = broadcast::channel(64);
        AppState {
            inner: Arc::new(RwLock::new(Inner::default())),
            config: Arc::new(config),
            sync_lock: Arc::new(Mutex::new(())),
            events: tx,
        }
    }
}
