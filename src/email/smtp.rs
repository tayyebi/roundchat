/// SMTP client.
///
/// Uses the `lettre` crate (async, tokio runtime) to send outgoing messages.
/// Each send opens a fresh connection so no persistent session is required.

use anyhow::{bail, Context, Result};
use lettre::message::{header::ContentType, Mailbox, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use crate::config::SmtpConfig;

/// A message to be sent.
pub struct OutboundMessage {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub body: String,
}

/// Send `msg` via SMTP using `config` and the provided credentials.
pub async fn send_message(
    config: &SmtpConfig,
    email: &str,
    password: &str,
    msg: OutboundMessage,
) -> Result<()> {
    let from: Mailbox = msg.from.parse().context("invalid From address")?;
    if msg.to.is_empty() {
        bail!("no recipients specified, cannot send email");
    }
    let subject = if msg.subject.is_empty() {
        "(no subject)".to_string()
    } else {
        msg.subject.clone()
    };

    let mut builder = Message::builder()
        .from(from.clone())
        .subject(subject);

    for addr in &msg.to {
        let mbox: Mailbox = addr.parse().context("invalid To address")?;
        builder = builder.to(mbox);
    }

    let email_msg = builder
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(msg.body.clone()),
        )
        .with_context(|| format!("failed to build email (to={:?})", msg.to))?;

    let creds = Credentials::new(email.to_string(), password.to_string());

    let transport = if config.port == 465 {
        // Port 465 = SMTPS (implicit TLS).
        let tls_params = TlsParameters::builder(config.host.clone())
            .dangerous_accept_invalid_certs(true)
            .dangerous_accept_invalid_hostnames(true)
            .build()
            .context("SMTP TLS parameters failed")?;
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
            .port(config.port)
            .tls(Tls::Wrapper(tls_params))
            .credentials(creds)
            .build()
    } else if config.tls {
        // Port 587 (or other) = STARTTLS.
        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
            .context("SMTP relay setup failed")?
            .port(config.port)
            .credentials(creds)
            .build()
    } else {
        // No encryption.
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
            .port(config.port)
            .credentials(creds)
            .build()
    };

    transport
        .send(email_msg)
        .await
        .context("SMTP send failed")?;

    Ok(())
}
