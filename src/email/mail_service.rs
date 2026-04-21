/// MailService — orchestrator.
///
/// Auto-selects IMAP or POP3 based on the presence of `ImapConfig`.
/// Exposes a single API used by the route handlers regardless of which
/// protocol is active underneath.

use anyhow::Result;
use std::collections::HashMap;
use crate::config::AppConfig;
use crate::email::{conversation::build_conversations, imap, pop3, smtp};
use crate::models::{Conversation, Message};

/// Fetch all messages and build conversations.
/// Returns the conversation list and the raw message list (for mark-read).
pub async fn fetch_conversations(
    config: &AppConfig,
    email: &str,
    password: &str,
) -> Result<Vec<Conversation>> {
    let (messages, subject_map) = fetch_messages_raw(config, email, password).await?;
    Ok(build_conversations(messages, email, &subject_map))
}

/// Fetch raw messages + subject map.
pub async fn fetch_messages_raw(
    config: &AppConfig,
    email: &str,
    password: &str,
) -> Result<(Vec<Message>, HashMap<String, String>)> {
    if let Some(imap_cfg) = &config.imap {
        imap::fetch_messages(imap_cfg.clone(), email.to_string(), password.to_string()).await
    } else if let Some(pop3_cfg) = &config.pop3 {
        pop3::fetch_messages(pop3_cfg.clone(), email.to_string(), password.to_string()).await
    } else {
        Ok((vec![], HashMap::new()))
    }
}

/// Send a message via SMTP.
pub async fn send_message(
    config: &AppConfig,
    email: &str,
    password: &str,
    to: Vec<String>,
    body: String,
    group_name: Option<String>,
) -> Result<()> {
    smtp::send_message(
        &config.smtp,
        email,
        password,
        smtp::OutboundMessage {
            from: email.to_string(),
            to,
            subject: group_name.unwrap_or_default(),
            body,
        },
    )
    .await
}

/// Mark a message as read (IMAP only; no-op for POP3).
pub async fn mark_read(
    config: &AppConfig,
    email: &str,
    password: &str,
    uid: &str,
) -> Result<()> {
    if let Some(imap_cfg) = &config.imap {
        imap::mark_read(
            imap_cfg.clone(),
            email.to_string(),
            password.to_string(),
            uid.to_string(),
        )
        .await?;
    }
    Ok(())
}
