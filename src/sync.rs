/// Coordinates protocol synchronization, durable cache writes, and UI state.

use anyhow::{bail, Context, Result};

use crate::email::{conversation::build_conversations, mail_service};
use crate::models::{Conversation, Message, Session};
use crate::state::{AppEvent, AppState};
use crate::storage;

fn account_session(inner: &crate::state::Inner, email: &str) -> Result<Session> {
    match &inner.account {
        Some(session) if session.email == email => Ok(session.clone()),
        _ => bail!("mailbox session is no longer active"),
    }
}

fn save_cache(state: &AppState, email: &str, cache: &crate::models::MailCache) {
    if let Err(error) = storage::save_mail_cache(&state.config.data_dir, email, cache) {
        // A successful SMTP/IMAP operation must not be reported as failed just
        // because an optional local cache write was unavailable.
        tracing::warn!("Could not persist local chat cache: {error}");
    }
}

/// Return the immediately available cached view, used as an offline fallback.
pub async fn cached_conversations(state: &AppState, email: &str) -> Result<Vec<Conversation>> {
    let inner = state.inner.read().await;
    account_session(&inner, email)?;
    Ok(inner.conversations.clone())
}

/// Refresh one account. `notify` is false for browser-initiated GET requests so
/// an SSE listener cannot create an endless fetch/event loop.
pub async fn refresh_conversations(
    state: &AppState,
    email: &str,
    notify: bool,
) -> Result<Vec<Conversation>> {
    let _guard = state.sync_lock.lock().await;
    let (session, mut cache) = {
        let inner = state.inner.read().await;
        (account_session(&inner, email)?, inner.cache.clone())
    };

    mail_service::sync_cache(
        &state.config,
        &session.email,
        &session.password,
        &mut cache,
    )
    .await
    .context("mailbox synchronization failed")?;
    let conversations = build_conversations(cache.messages.clone(), email);

    {
        let mut inner = state.inner.write().await;
        account_session(&inner, email)?;
        inner.cache = cache.clone();
        inner.conversations = conversations.clone();
    }
    save_cache(state, email, &cache);
    if notify {
        state.events.send(AppEvent::ConversationsUpdated).ok();
    }
    Ok(conversations)
}

/// Persist an outgoing message only after SMTP accepted it, then return its
/// fully built conversation so the composer can replace its temporary state.
pub async fn record_outgoing(
    state: &AppState,
    email: &str,
    message: Message,
) -> Result<Conversation> {
    let _guard = state.sync_lock.lock().await;
    let conversation_id = message.conversation_id.clone();
    let conversations = {
        let mut inner = state.inner.write().await;
        account_session(&inner, email)?;
        inner.cache.messages.push(message);
        inner.conversations = build_conversations(inner.cache.messages.clone(), email);
        inner.conversations.clone()
    };
    let cache = state.inner.read().await.cache.clone();
    save_cache(state, email, &cache);

    let conversation = conversations
        .into_iter()
        .find(|conversation| Some(&conversation.id) == conversation_id.as_ref())
        .context("outgoing conversation was not built")?;
    state.events.send(AppEvent::ConversationsUpdated).ok();
    Ok(conversation)
}

/// Mark a batch of remote IMAP UIDs and the durable local cache in one pass.
pub async fn mark_messages_read(state: &AppState, email: &str, uids: &[u32]) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    let _guard = state.sync_lock.lock().await;
    let (session, imap_uids) = {
        let inner = state.inner.read().await;
        let session = account_session(&inner, email)?;
        let imap_uids: Vec<u32> = inner
            .cache
            .messages
            .iter()
            .filter(|message| {
                message.source == "imap"
                    && message.uid.map(|uid| uids.contains(&uid)).unwrap_or(false)
            })
            .filter_map(|message| message.uid)
            .collect();
        (session, imap_uids)
    };

    if !imap_uids.is_empty() {
        mail_service::mark_read(
            &state.config,
            &session.email,
            &session.password,
            &imap_uids,
        )
        .await?;
    }

    let cache = {
        let mut inner = state.inner.write().await;
        account_session(&inner, email)?;
        for message in &mut inner.cache.messages {
            if message.uid.map(|uid| uids.contains(&uid)).unwrap_or(false) {
                message.read = true;
            }
        }
        inner.conversations = build_conversations(inner.cache.messages.clone(), email);
        inner.cache.clone()
    };
    save_cache(state, email, &cache);
    state.events.send(AppEvent::ConversationsUpdated).ok();
    Ok(())
}
