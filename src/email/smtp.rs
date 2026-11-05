/// SMTP client.
///
/// Outgoing chat messages are ordinary plain-text emails with standard
/// threading headers plus a stable RoundChat conversation header.

use anyhow::{bail, Context, Result};
use lettre::message::header::{ContentType, Header, HeaderName, HeaderValue};
use lettre::message::{Mailbox, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::SmtpConfig;

#[derive(Clone)]
struct RoundChatConversationId(String);

impl Header for RoundChatConversationId {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("X-RoundChat-Conversation-ID")
    }

    fn parse(value: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self(value.to_string()))
    }

    fn display(&self) -> HeaderValue {
        HeaderValue::new(Self::name(), self.0.clone())
    }
}

/// A message to be sent.
pub struct OutboundMessage {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub body: String,
    pub message_id: String,
    pub conversation_id: String,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
}

fn bracketed(id: &str) -> String {
    format!("<{}>", id.trim().trim_matches(|ch| ch == '<' || ch == '>'))
}

/// Send `msg` and return its RFC 5322 representation so callers may optionally
/// append it to an IMAP Sent folder.
pub async fn send_message(
    config: &SmtpConfig,
    email: &str,
    password: &str,
    msg: OutboundMessage,
) -> Result<Vec<u8>> {
    let from: Mailbox = msg.from.parse().context("invalid From address")?;
    if msg.to.is_empty() {
        bail!("no recipients specified, cannot send email");
    }

    let subject = if msg.subject.trim().is_empty() {
        "RoundChat chat".to_string()
    } else {
        msg.subject
    };
    let mut builder = Message::builder()
        .from(from)
        .subject(subject)
        .date_now()
        .message_id(Some(bracketed(&msg.message_id)))
        .header(RoundChatConversationId(msg.conversation_id));

    for address in &msg.to {
        let mailbox: Mailbox = address.parse().context("invalid To address")?;
        builder = builder.to(mailbox);
    }
    if let Some(in_reply_to) = msg.in_reply_to {
        builder = builder.in_reply_to(bracketed(&in_reply_to));
    }
    if !msg.references.is_empty() {
        let references = msg
            .references
            .iter()
            .map(|id| bracketed(id))
            .collect::<Vec<String>>()
            .join(" ");
        builder = builder.references(references);
    }

    let email_message = builder
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(msg.body),
        )
        .context("failed to build outgoing email")?;
    let raw = email_message.formatted();
    let credentials = Credentials::new(email.to_string(), password.to_string());

    let transport = if config.tls {
        let mut parameters = TlsParameters::builder(config.host.clone());
        if config.tls_insecure {
            parameters = parameters
                .dangerous_accept_invalid_certs(true)
                .dangerous_accept_invalid_hostnames(true);
        }
        let parameters = parameters.build().context("SMTP TLS parameters failed")?;
        let mode = if config.port == 465 {
            Tls::Wrapper(parameters)
        } else {
            Tls::Required(parameters)
        };
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
            .port(config.port)
            .tls(mode)
            .credentials(credentials)
            .build()
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
            .port(config.port)
            .credentials(credentials)
            .build()
    };

    transport
        .send(email_message)
        .await
        .context("SMTP send failed")?;
    Ok(raw)
}
