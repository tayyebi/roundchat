/// IMAP client.
///
/// Uses BODY.PEEK and cached UIDs so synchronization does not download every
/// message body or implicitly mark mail as read. INBOX remains the only folder
/// required by the chat-focused application.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use async_imap::Client;
use async_imap::extensions::idle::IdleResponse;
use futures::StreamExt;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_native_tls::TlsStream;

use crate::config::ImapConfig;
use crate::email::parser::parse_raw_message;
use crate::models::Message;

const IDLE_REFRESH_SECS: u64 = 28 * 60;
const UID_CHUNK_SIZE: usize = 200;

/// A single stream enum lets IMAP_TLS=false work without duplicating the
/// entire client for plaintext and TLS sessions.
enum ImapStream {
    Plain(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl fmt::Debug for ImapStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Plain(_) => "ImapStream::Plain",
            Self::Tls(_) => "ImapStream::Tls",
        })
    }
}

impl AsyncRead for ImapStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_read(cx, buffer),
            Self::Tls(stream) => Pin::new(stream).poll_read(cx, buffer),
        }
    }
}

impl AsyncWrite for ImapStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_write(cx, buffer),
            Self::Tls(stream) => Pin::new(stream).poll_write(cx, buffer),
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_flush(cx),
            Self::Tls(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Tls(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

type ImapSession = async_imap::Session<ImapStream>;

#[derive(Debug, Default)]
pub struct ImapUpdate {
    pub uid_validity: Option<u32>,
    pub present_uids: HashSet<u32>,
    pub read_by_uid: HashMap<u32, bool>,
    pub messages: Vec<Message>,
}

async fn open_session(config: &ImapConfig, email: &str, password: &str) -> Result<ImapSession> {
    let tcp = TcpStream::connect((config.host.as_str(), config.port))
        .await
        .context("IMAP TCP connect failed")?;

    let stream = if config.tls {
        let mut builder = tokio_native_tls::native_tls::TlsConnector::builder();
        if config.tls_insecure {
            builder.danger_accept_invalid_certs(true);
            builder.danger_accept_invalid_hostnames(true);
        }
        let native_context = builder.build().context("TLS connector build failed")?;
        let context = tokio_native_tls::TlsConnector::from(native_context);
        let tls = context
            .connect(config.host.as_str(), tcp)
            .await
            .context("IMAP TLS handshake failed")?;
        ImapStream::Tls(tls)
    } else {
        ImapStream::Plain(tcp)
    };

    let client = Client::new(stream);
    client
        .login(email, password)
        .await
        .map_err(|(error, _)| anyhow!("IMAP login failed: {error}"))
}

/// Authenticate without downloading the user's mailbox.
pub async fn verify_credentials(config: &ImapConfig, email: &str, password: &str) -> Result<()> {
    let session = open_session(config, email, password).await?;
    let mut session = session;
    session.logout().await.context("IMAP logout failed")?;
    Ok(())
}

fn chunks(uids: &HashSet<u32>) -> Vec<Vec<u32>> {
    let mut sorted: Vec<u32> = uids.iter().copied().collect();
    sorted.sort_unstable();
    sorted
        .chunks(UID_CHUNK_SIZE)
        .map(|chunk| chunk.to_vec())
        .collect()
}

fn uid_list(uids: &[u32]) -> String {
    uids.iter()
        .map(u32::to_string)
        .collect::<Vec<String>>()
        .join(",")
}

/// Fetch flags for existing mail and bodies only for UIDs absent from the
/// local cache. The caller reconciles deletions with `present_uids`.
pub async fn sync_messages(
    config: &ImapConfig,
    email: &str,
    password: &str,
    cached_uids: &HashSet<u32>,
    previous_uid_validity: Option<u32>,
) -> Result<ImapUpdate> {
    let mut session = open_session(config, email, password).await?;
    let mailbox = session
        .select("INBOX")
        .await
        .context("IMAP SELECT INBOX failed")?;
    let uid_validity = mailbox.uid_validity;
    let present_uids = session
        .uid_search("ALL")
        .await
        .context("IMAP UID SEARCH failed")?;
    let mut update = ImapUpdate {
        uid_validity,
        present_uids: present_uids.clone(),
        ..ImapUpdate::default()
    };

    if present_uids.is_empty() {
        session.logout().await.ok();
        return Ok(update);
    }

    for chunk in chunks(&present_uids) {
        let mut fetch_stream = session
            .uid_fetch(uid_list(&chunk), "(UID FLAGS)")
            .await
            .context("IMAP UID FLAGS FETCH failed")?;
        while let Some(fetch) = fetch_stream.next().await {
            let fetch = fetch.context("IMAP FLAGS FETCH stream error")?;
            if let Some(uid) = fetch.uid {
                let read = fetch
                    .flags()
                    .any(|flag| matches!(flag, async_imap::types::Flag::Seen));
                update.read_by_uid.insert(uid, read);
            }
        }
        drop(fetch_stream);
    }

    let cache_is_valid = previous_uid_validity == uid_validity;
    let new_uids: HashSet<u32> = if cache_is_valid {
        present_uids
            .difference(cached_uids)
            .copied()
            .collect::<HashSet<u32>>()
    } else {
        present_uids.clone()
    };

    for chunk in chunks(&new_uids) {
        let mut fetch_stream = session
            .uid_fetch(uid_list(&chunk), "(UID BODY.PEEK[] FLAGS)")
            .await
            .context("IMAP UID BODY FETCH failed")?;
        while let Some(fetch) = fetch_stream.next().await {
            let fetch = fetch.context("IMAP BODY FETCH stream error")?;
            let Some(uid) = fetch.uid else {
                continue;
            };
            let Some(raw_bytes) = fetch.body() else {
                tracing::warn!("IMAP message UID={uid} has no body");
                continue;
            };
            match parse_raw_message(raw_bytes, &format!("imap:{uid}")) {
                Ok(mut message) => {
                    message.uid = Some(uid);
                    message.source = "imap".to_string();
                    message.read = fetch.flags().any(|flag| {
                        matches!(flag, async_imap::types::Flag::Seen)
                    });
                    update.messages.push(message);
                }
                Err(error) => {
                    tracing::warn!("Could not parse message UID={uid}: {error}");
                }
            }
        }
        drop(fetch_stream);
    }

    session.logout().await.ok();
    Ok(update)
}

/// Mark a message UID as Seen.
pub async fn mark_read(
    config: &ImapConfig,
    email: &str,
    password: &str,
    uids: &[u32],
) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    let uid_set: HashSet<u32> = uids.iter().copied().collect();
    let mut session = open_session(config, email, password).await?;
    session.select("INBOX").await.context("SELECT INBOX")?;
    for chunk in chunks(&uid_set) {
        let mut store_stream = session
            .uid_store(uid_list(&chunk), "+FLAGS (\\Seen)")
            .await
            .context("UID STORE failed")?;
        while store_stream.next().await.is_some() {}
        drop(store_stream);
    }
    session.logout().await.ok();
    Ok(())
}

/// Append an outgoing RFC 5322 message to a configured Sent folder.
pub async fn append_message(
    config: &ImapConfig,
    email: &str,
    password: &str,
    folder: &str,
    raw: &[u8],
) -> Result<()> {
    let mut session = open_session(config, email, password).await?;
    session
        .append(folder, Some("(\\Seen)"), None, raw)
        .await
        .with_context(|| format!("IMAP APPEND to {folder} failed"))?;
    session.logout().await.ok();
    Ok(())
}

/// Run an RFC 2177 IMAP IDLE loop for INBOX.
pub async fn idle_loop(
    config: &ImapConfig,
    email: &str,
    password: &str,
    on_change: impl Fn(),
) -> Result<()> {
    let mut session = open_session(config, email, password).await?;
    session
        .select("INBOX")
        .await
        .context("IMAP SELECT INBOX failed")?;

    loop {
        let mut handle = session.idle();
        handle.init().await.context("IMAP IDLE init failed")?;
        let (wait_future, _stop) =
            handle.wait_with_timeout(Duration::from_secs(IDLE_REFRESH_SECS));
        let response = wait_future.await.context("IMAP IDLE wait failed")?;
        session = handle.done().await.context("IMAP IDLE done failed")?;

        match response {
            IdleResponse::NewData(_) => on_change(),
            IdleResponse::Timeout => {
                tracing::debug!("IMAP IDLE: re-issuing IDLE (28-min refresh)");
            }
            IdleResponse::ManualInterrupt => break,
        }
    }

    session.logout().await.ok();
    Ok(())
}
