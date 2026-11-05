/// Data models shared across the application.

use serde::{Deserialize, Serialize};

/// A file attachment linked via WebDAV.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attachment {
    pub url: String,
    pub mime_type: String,
    pub filename: String,
    pub size: i64,
}

/// A single chat message derived from one email.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// RFC 5322 Message-ID without angle brackets.
    pub id: String,
    /// IMAP UID or POP3 message number. Local outgoing messages have no UID.
    #[serde(default)]
    pub uid: Option<u32>,
    /// Remote protocol (`imap`/`pop3`), or empty for a local outgoing message.
    #[serde(default)]
    pub source: String,
    /// Sender email address (lower-cased).
    pub from: String,
    /// Recipient addresses (To + CC, lower-cased).
    pub to: Vec<String>,
    /// ISO-8601 date string.
    pub date: String,
    /// Plain-text body, without RoundChat attachment metadata.
    pub body: String,
    /// File attachments.
    pub attachments: Vec<Attachment>,
    /// Whether the local user has read this message.
    pub read: bool,
    /// Original email subject, retained for interoperable replies.
    #[serde(default)]
    pub subject: String,
    /// Stable RoundChat conversation ID, when supplied by another RoundChat client.
    #[serde(default)]
    pub conversation_id: Option<String>,
    /// RFC 5322 In-Reply-To value without angle brackets.
    #[serde(default)]
    pub in_reply_to: Option<String>,
    /// RFC 5322 References values without angle brackets.
    #[serde(default)]
    pub references: Vec<String>,
}

/// An email thread presented as a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// Stable conversation key shared by RoundChat clients.
    pub id: String,
    /// Null for one-to-one conversations; normalized email Subject for group chats.
    pub group_name: Option<String>,
    /// Original/normalized subject used when sending an email reply.
    pub subject: Option<String>,
    /// All participant addresses (including local user).
    pub participants: Vec<String>,
    /// Messages ordered oldest-first.
    pub messages: Vec<Message>,
    /// Most recent message, or None if the thread is empty.
    pub last_message: Option<Message>,
    /// Count of unread incoming messages in this thread.
    pub unread_count: usize,
}

/// A contact sourced from CardDAV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    /// vCard UID.
    pub id: String,
    /// Full display name.
    pub display_name: String,
    /// Primary email address.
    pub email: String,
    /// Optional phone number.
    pub phone: Option<String>,
    /// Optional avatar URL.
    pub avatar_url: Option<String>,
}

/// A file stored on WebDAV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteFile {
    pub name: String,
    pub size: u64,
    pub mime_type: String,
    pub url: String,
    pub last_modified: String,
}

/// An authenticated mailbox account held in memory.
///
/// The password is intentionally not serializable and is never persisted.
#[derive(Clone)]
pub struct Session {
    pub email: String,
    pub password: String,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("email", &self.email)
            .field("password", &"[redacted]")
            .finish()
    }
}

/// Durable per-account mail cache. Remote bodies are fetched incrementally;
/// local outgoing messages remain here even though they are not in INBOX.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MailCache {
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub uid_validity: Option<u32>,
    #[serde(default)]
    pub protocol: String,
}
