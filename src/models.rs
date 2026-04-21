/// Data models shared across the application.

use serde::{Deserialize, Serialize};

/// A file attachment linked via WebDAV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub url: String,
    pub mime_type: String,
    pub filename: String,
    pub size: i64,
}

/// A single chat message derived from one email.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Globally unique identifier (Message-ID header value).
    pub id: String,
    /// Sender email address (lower-cased).
    pub from: String,
    /// Recipient addresses (To + CC, lower-cased).
    pub to: Vec<String>,
    /// ISO-8601 date string.
    pub date: String,
    /// Plain-text body.
    pub body: String,
    /// File attachments.
    pub attachments: Vec<Attachment>,
    /// Whether the local user has read this message.
    pub read: bool,
}

/// An email thread presented as a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// Stable thread key (hash of sorted participants + optional group name).
    pub id: String,
    /// Null for one-to-one conversations; email Subject for group chats.
    pub group_name: Option<String>,
    /// All participant addresses (including local user).
    pub participants: Vec<String>,
    /// Messages ordered oldest-first.
    pub messages: Vec<Message>,
    /// Most recent message, or None if the thread is empty.
    pub last_message: Option<Message>,
    /// Count of unread messages in this thread.
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

/// An authenticated user session held in memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub email: String,
    pub password: String,
}
