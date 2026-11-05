/// MailService — protocol adapters and cache reconciliation.

use std::collections::{HashMap, HashSet};

use anyhow::{bail, Result};

use crate::config::AppConfig;
use crate::email::{imap, pop3, smtp};
use crate::models::{MailCache, Message};

/// Authenticate using the configured incoming protocol without downloading
/// every message merely to verify a login.
pub async fn verify_credentials(
    config: &AppConfig,
    email: &str,
    password: &str,
) -> Result<()> {
    if let Some(imap_config) = &config.imap {
        imap::verify_credentials(imap_config, email, password).await
    } else if let Some(pop3_config) = &config.pop3 {
        pop3::verify_credentials(pop3_config.clone(), email.to_string(), password.to_string()).await
    } else {
        bail!("configure IMAP_HOST or POP3_HOST before signing in")
    }
}

fn prepare_protocol(cache: &mut MailCache, protocol: &str) {
    if !cache.protocol.is_empty() && cache.protocol != protocol {
        // Keep locally sent history even if the incoming protocol changes.
        cache.messages.retain(|message| message.source.is_empty());
        cache.uid_validity = None;
    }
    cache.protocol = protocol.to_string();
}

fn upsert_message(cache: &mut MailCache, message: Message) {
    if let Some(existing) = cache
        .messages
        .iter_mut()
        .find(|existing| existing.id == message.id)
    {
        // A local outgoing copy may already exist when a provider also places
        // that Message-ID in INBOX. Never regress its local read state.
        let was_read = existing.read;
        *existing = message;
        existing.read |= was_read;
    } else {
        cache.messages.push(message);
    }
}

fn merge_imap(cache: &mut MailCache, update: imap::ImapUpdate) {
    if cache.uid_validity != update.uid_validity {
        cache.messages.retain(|message| message.source != "imap");
    }
    cache.uid_validity = update.uid_validity;

    cache.messages.retain(|message| {
        message.source != "imap"
            || message
                .uid
                .map(|uid| update.present_uids.contains(&uid))
                .unwrap_or(false)
    });
    for message in &mut cache.messages {
        if message.source == "imap" {
            if let Some(read) = message.uid.and_then(|uid| update.read_by_uid.get(&uid)) {
                message.read = *read;
            }
        }
    }
    for message in update.messages {
        upsert_message(cache, message);
    }
}

fn merge_pop3(cache: &mut MailCache, messages: Vec<Message>) {
    let previous_read: HashMap<String, bool> = cache
        .messages
        .iter()
        .filter(|message| message.source == "pop3")
        .map(|message| (message.id.clone(), message.read))
        .collect();
    cache.messages.retain(|message| message.source != "pop3");

    for mut message in messages {
        if let Some(read) = previous_read.get(&message.id) {
            message.read = *read;
        }
        upsert_message(cache, message);
    }
}

/// Synchronize new data into `cache`. IMAP fetches flags for existing UIDs but
/// downloads only missing bodies; POP3 remains a full-mailbox fallback.
pub async fn sync_cache(
    config: &AppConfig,
    email: &str,
    password: &str,
    cache: &mut MailCache,
) -> Result<()> {
    if let Some(imap_config) = &config.imap {
        prepare_protocol(cache, "imap");
        let cached_uids: HashSet<u32> = cache
            .messages
            .iter()
            .filter(|message| message.source == "imap")
            .filter_map(|message| message.uid)
            .collect();
        let update = imap::sync_messages(
            imap_config,
            email,
            password,
            &cached_uids,
            cache.uid_validity,
        )
        .await?;
        merge_imap(cache, update);
        Ok(())
    } else if let Some(pop3_config) = &config.pop3 {
        prepare_protocol(cache, "pop3");
        let messages = pop3::fetch_messages(
            pop3_config.clone(),
            email.to_string(),
            password.to_string(),
        )
        .await?;
        merge_pop3(cache, messages);
        Ok(())
    } else {
        bail!("no incoming mail protocol is configured")
    }
}

/// Send a chat email and optionally save a copy to the configured IMAP Sent
/// folder. SMTP success is not rolled back if the optional APPEND fails.
pub async fn send_message(
    config: &AppConfig,
    email: &str,
    password: &str,
    message: smtp::OutboundMessage,
) -> Result<()> {
    let raw = smtp::send_message(&config.smtp, email, password, message).await?;

    if let Some(imap_config) = &config.imap {
        if let Some(folder) = &imap_config.sent_folder {
            if let Err(error) = imap::append_message(
                imap_config,
                email,
                password,
                folder,
                &raw,
            )
            .await
            {
                tracing::warn!("Message sent, but saving to {folder} failed: {error}");
            }
        }
    }
    Ok(())
}

/// Mark a remote IMAP message as read. POP3 read state is cache-local.
pub async fn mark_read(
    config: &AppConfig,
    email: &str,
    password: &str,
    uids: &[u32],
) -> Result<()> {
    if let Some(imap_config) = &config.imap {
        imap::mark_read(imap_config, email, password, uids).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Attachment;

    fn message(id: &str, source: &str, uid: Option<u32>) -> Message {
        Message {
            id: id.to_string(),
            uid,
            source: source.to_string(),
            from: "alice@example.com".to_string(),
            to: vec!["me@example.com".to_string()],
            date: "2025-01-01T00:00:00+00:00".to_string(),
            body: String::new(),
            attachments: Vec::<Attachment>::new(),
            read: false,
            subject: String::new(),
            conversation_id: None,
            in_reply_to: None,
            references: vec![],
        }
    }

    #[test]
    fn imap_reconciliation_preserves_local_outgoing_messages() {
        let mut cache = MailCache {
            messages: vec![
                message("local", "", None),
                message("remote", "imap", Some(1)),
            ],
            uid_validity: Some(10),
            protocol: "imap".to_string(),
        };
        let update = imap::ImapUpdate {
            uid_validity: Some(10),
            present_uids: HashSet::new(),
            read_by_uid: HashMap::new(),
            messages: vec![],
        };

        merge_imap(&mut cache, update);
        assert_eq!(cache.messages.len(), 1);
        assert_eq!(cache.messages[0].id, "local");
    }

    #[test]
    fn protocol_changes_only_drop_remote_messages() {
        let mut cache = MailCache {
            messages: vec![message("local", "", None), message("remote", "pop3", Some(1))],
            uid_validity: None,
            protocol: "pop3".to_string(),
        };
        prepare_protocol(&mut cache, "imap");
        assert_eq!(cache.messages.len(), 1);
        assert_eq!(cache.messages[0].id, "local");
    }
}
