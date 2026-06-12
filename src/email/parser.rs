/// RFC 5322 email parser.
///
/// Converts a raw email string into the app's Message model.
/// Uses the `mailparse` crate for header and MIME parsing.

use anyhow::{Context, Result};
use mailparse::{parse_mail, MailHeaderMap};
use crate::models::{Attachment, Message};

/// Parse a raw RFC 5322 email string into a Message.
/// `fallback_id` is used when the Message-ID header is missing.
pub fn parse_raw_message(raw: &str, fallback_id: &str) -> Result<Message> {
    let parsed = parse_mail(raw.as_bytes()).context("failed to parse email")?;
    let headers = &parsed.headers;

    let message_id = headers
        .get_first_value("Message-ID")
        .unwrap_or_default()
        .trim_matches(|c| c == '<' || c == '>')
        .to_string();
    let id = if message_id.is_empty() {
        tracing::warn!("Message missing Message-ID header, using fallback: {fallback_id}");
        fallback_id.to_string()
    } else {
        message_id
    };

    let from = headers
        .get_first_value("From")
        .unwrap_or_default();
    if from.is_empty() {
        tracing::warn!("Message {id} missing From header");
    }
    let from = extract_first_address(&from);

    let to_header = headers.get_first_value("To").unwrap_or_default();
    let cc_header = headers.get_first_value("CC").unwrap_or_default();
    let mut to = split_addresses(&to_header);
    to.extend(split_addresses(&cc_header));

    let date_str = headers.get_first_value("Date").unwrap_or_default();
    let date = parse_date(&date_str);

    let body = extract_body(&parsed);
    let attachments = extract_attachments(&body);

    Ok(Message {
        id,
        from,
        to,
        date,
        body,
        attachments,
        read: false,
    })
}

/// Split a comma-separated address list, extracting the addr-spec.
fn split_addresses(field: &str) -> Vec<String> {
    if field.is_empty() {
        return vec![];
    }
    field
        .split(',')
        .map(|a| extract_first_address(a))
        .filter(|a| !a.is_empty())
        .collect()
}

/// Extract the bare email address from a display-name+addr-spec token.
fn extract_first_address(raw: &str) -> String {
    let raw = raw.trim();
    // "Display Name <email@host>" → email@host
    if let Some(start) = raw.find('<') {
        if let Some(end) = raw.find('>') {
            return raw[start + 1..end].trim().to_lowercase();
        }
    }
    // Plain address
    raw.to_lowercase()
}

/// Extract the plain-text body from a possibly multipart email.
fn extract_body(parsed: &mailparse::ParsedMail) -> String {
    if parsed.subparts.is_empty() {
        match parsed.get_body() {
            Ok(body) => return body,
            Err(e) => tracing::warn!("Failed to decode email body: {e}"),
        }
        return String::new();
    }
    // Prefer text/plain part in multipart messages.
    for part in &parsed.subparts {
        let ct = part
            .headers
            .get_first_value("Content-Type")
            .unwrap_or_default();
        if ct.to_lowercase().starts_with("text/plain") {
            match part.get_body() {
                Ok(body) => return body,
                Err(e) => tracing::warn!("Failed to decode text/plain part: {e}"),
            }
        }
    }
    // Fallback: first part body.
    match parsed.subparts[0].get_body() {
        Ok(body) => body,
        Err(e) => {
            tracing::warn!("Failed to decode fallback body part: {e}");
            String::new()
        }
    }
}

/// Extract WebDAV-linked attachments from the body text.
/// Messages produced by roundchat embed attachment info as plain-text lines:
///   [file: <filename> | <url>]
fn extract_attachments(body: &str) -> Vec<Attachment> {
    let mut attachments = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.starts_with("[file:") && line.ends_with(']') {
            let inner = line[6..line.len() - 1].trim().to_string();
            let parts: Vec<&str> = inner.splitn(3, '|').collect();
            if parts.len() == 3 {
                attachments.push(Attachment {
                    filename: parts[0].trim().to_string(),
                    url: parts[1].trim().to_string(),
                    mime_type: parts[2].trim().to_string(),
                    size: -1,
                });
            }
        }
    }
    attachments
}

/// Parse an RFC 2822 date string to ISO-8601.
fn parse_date(date_str: &str) -> String {
    // Try parsing with mailparse's date support.
    if let Ok(dt) = mailparse::dateparse(date_str) {
        // dt is a Unix timestamp (i64)
        let naive = chrono::DateTime::from_timestamp(dt, 0)
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
        return naive.to_rfc3339();
    }
    chrono::Utc::now().to_rfc3339()
}
