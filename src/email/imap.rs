/// IMAP client.
///
/// Connects to an IMAP server over TLS using async-imap + tokio-native-tls.
/// Fetches messages, marks them as read, and drives an RFC 2177 IDLE loop
/// so the caller is notified of new mail in real time with zero polling lag.

use anyhow::{anyhow, Context, Result};
use async_imap::Client;
use async_imap::extensions::idle::IdleResponse;
use futures::StreamExt;
use std::collections::HashMap;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_native_tls::TlsStream;
use crate::config::ImapConfig;
use crate::email::parser::parse_raw_message;
use crate::models::Message;

/// RFC 2177 recommends clients re-issue IDLE at least every 29 minutes.
/// We use 28 minutes to give a small margin.
const IDLE_REFRESH_SECS: u64 = 28 * 60;

type ImapSession = async_imap::Session<TlsStream<TcpStream>>;

/// Open an authenticated IMAP session over TLS.
async fn open_session(
    config: &ImapConfig,
    email: &str,
    password: &str,
) -> Result<ImapSession> {
    let tcp = TcpStream::connect((config.host.as_str(), config.port))
        .await
        .context("IMAP TCP connect failed")?;

    let native_cx = tokio_native_tls::native_tls::TlsConnector::builder()
        .build()
        .context("TLS connector build failed")?;
    let cx = tokio_native_tls::TlsConnector::from(native_cx);
    let tls = cx
        .connect(config.host.as_str(), tcp)
        .await
        .context("IMAP TLS handshake failed")?;

    let client = Client::new(tls);
    let session = client
        .login(email, password)
        .await
        .map_err(|(e, _)| anyhow!("IMAP login failed: {e}"))?;

    Ok(session)
}

/// Fetch all messages from INBOX and return them with a subject map.
pub async fn fetch_messages(
    config: ImapConfig,
    email: String,
    password: String,
) -> Result<(Vec<Message>, HashMap<String, String>)> {
    let mut session = open_session(&config, &email, &password).await?;
    session
        .select("INBOX")
        .await
        .context("IMAP SELECT INBOX failed")?;

    let uid_set = session
        .uid_search("ALL")
        .await
        .context("IMAP UID SEARCH failed")?;

    if uid_set.is_empty() {
        session.logout().await.ok();
        return Ok((vec![], HashMap::new()));
    }

    // Build a comma-separated UID list.
    let uid_list: Vec<String> = uid_set.iter().map(|u| u.to_string()).collect();
    let uid_range = uid_list.join(",");

    let mut fetch_stream = session
        .uid_fetch(&uid_range, "(RFC822 ENVELOPE FLAGS)")
        .await
        .context("IMAP UID FETCH failed")?;

    let mut messages = Vec::new();
    let mut subject_map = HashMap::new();

    while let Some(fetch_result) = fetch_stream.next().await {
        let fetch = fetch_result.context("IMAP FETCH stream error")?;
        let uid = fetch.uid.map(|u| u.to_string()).unwrap_or_default();

        if let Some(raw_bytes) = fetch.body() {
            let raw = String::from_utf8_lossy(raw_bytes);
            match parse_raw_message(&raw, &uid) {
                Ok(mut msg) => {
                    // Extract subject from envelope for group-name use.
                    if let Some(env) = fetch.envelope() {
                        if let Some(subj_bytes) = &env.subject {
                            let subj = String::from_utf8_lossy(subj_bytes).to_string();
                            let subj = decode_rfc2047(&subj);
                            subject_map.insert(msg.id.clone(), subj);
                        }
                    }
                    // Check \Seen flag.
                    msg.read = fetch
                        .flags()
                        .any(|f| matches!(f, async_imap::types::Flag::Seen));
                    messages.push(msg);
                }
                Err(e) => {
                    tracing::warn!("Could not parse message UID={uid}: {e}");
                }
            }
        }
    }
    drop(fetch_stream);

    session.logout().await.ok();
    Ok((messages, subject_map))
}

/// Mark a message UID as \Seen.
pub async fn mark_read(
    config: ImapConfig,
    email: String,
    password: String,
    uid: String,
) -> Result<()> {
    let mut session = open_session(&config, &email, &password).await?;
    session.select("INBOX").await.context("SELECT INBOX")?;
    let mut store_stream = session
        .uid_store(&uid, "+FLAGS (\\Seen)")
        .await
        .context("UID STORE failed")?;
    while store_stream.next().await.is_some() {}
    drop(store_stream);
    session.logout().await.ok();
    Ok(())
}

/// Run an RFC 2177 IMAP IDLE loop.
///
/// Keeps a persistent IMAP connection open.  The server pushes an unsolicited
/// `EXISTS` or `RECENT` response the instant new mail arrives; we translate
/// that into a call to `on_change` so the caller can re-fetch and push an SSE
/// event to connected browser tabs.
///
/// To comply with RFC 2177 we re-issue `IDLE` every 28 minutes so the server
/// never silently drops the connection due to inactivity.
///
/// This function runs until the connection fails, at which point it returns
/// the error so the caller can reconnect and restart the loop.
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
        // Enter IDLE: `session.idle()` consumes the session and returns a Handle.
        let mut handle = session.idle();
        handle.init().await.context("IMAP IDLE init failed")?;

        let (wait_fut, _stop) =
            handle.wait_with_timeout(Duration::from_secs(IDLE_REFRESH_SECS));

        let response = wait_fut.await.context("IMAP IDLE wait failed")?;

        // Exit IDLE and recover the session so we can use it again.
        session = handle.done().await.context("IMAP IDLE done failed")?;

        match response {
            IdleResponse::NewData(_) => {
                // The server reported a mailbox change (new mail, flag update,
                // expunge, etc.).  Notify the caller.
                tracing::debug!("IMAP IDLE: new data from server");
                on_change();
            }
            IdleResponse::Timeout => {
                // 28-minute timer fired — re-issue IDLE immediately.
                tracing::debug!("IMAP IDLE: re-issuing IDLE (28-min refresh)");
            }
            IdleResponse::ManualInterrupt => {
                // The stop token was triggered — clean shutdown requested.
                tracing::debug!("IMAP IDLE: manual interrupt, stopping");
                break;
            }
        }
    }

    session.logout().await.ok();
    Ok(())
}

/// Decode a simple RFC 2047 encoded-word (best-effort, handles UTF-8/base64).
fn decode_rfc2047(input: &str) -> String {
    let re = regex::Regex::new(r"=\?([^?]+)\?([BbQq])\?([^?]*)\?=").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        let encoding = caps[2].to_uppercase();
        let text = &caps[3];
        if encoding == "B" {
            use base64::Engine;
            if let Ok(bytes) =
                base64::engine::general_purpose::STANDARD.decode(text)
            {
                return String::from_utf8_lossy(&bytes).to_string();
            }
        }
        text.to_string()
    })
    .to_string()
}
