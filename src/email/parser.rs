/// RFC 5322 email parser.
///
/// Converts a raw email into a chat message while preserving the standard
/// threading headers and RoundChat's explicit conversation identifier.

use anyhow::{Context, Result};
use mailparse::{parse_mail, MailHeaderMap, ParsedMail};

use crate::models::{Attachment, Message};

/// Parse a raw RFC 5322 email into a Message.
/// `fallback_id` is used when the Message-ID header is missing.
pub fn parse_raw_message(raw: &[u8], fallback_id: &str) -> Result<Message> {
    let parsed = parse_mail(raw).context("failed to parse email")?;
    let headers = &parsed.headers;

    let message_id = normalize_message_id(
        &headers.get_first_value("Message-ID").unwrap_or_default(),
    );
    let id = if message_id.is_empty() {
        tracing::warn!("Message missing Message-ID header, using fallback: {fallback_id}");
        fallback_id.to_string()
    } else {
        message_id
    };

    let from = headers.get_first_value("From").unwrap_or_default();
    if from.is_empty() {
        tracing::warn!("Message {id} missing From header");
    }
    let from = extract_first_address(&from);

    let to_header = headers.get_first_value("To").unwrap_or_default();
    let cc_header = headers.get_first_value("CC").unwrap_or_default();
    let mut to = split_addresses(&to_header);
    to.extend(split_addresses(&cc_header));
    to.sort();
    to.dedup();

    let date_str = headers.get_first_value("Date").unwrap_or_default();
    let date = parse_date(&date_str);
    let subject = headers.get_first_value("Subject").unwrap_or_default();
    let raw_body = extract_body(&parsed);
    let (body, attachments) = extract_attachments(&raw_body);

    let in_reply_to = headers
        .get_first_value("In-Reply-To")
        .and_then(|value| parse_message_ids(&value).last().cloned());
    let references = headers
        .get_first_value("References")
        .map(|value| parse_message_ids(&value))
        .unwrap_or_default();
    let conversation_id = headers
        .get_first_value("X-RoundChat-Conversation-ID")
        .and_then(|value| {
            let value = value.trim().to_string();
            valid_conversation_id(&value).then_some(value)
        });

    Ok(Message {
        id,
        uid: None,
        source: String::new(),
        from,
        to,
        date,
        body,
        attachments,
        read: false,
        subject,
        conversation_id,
        in_reply_to,
        references,
    })
}

/// RoundChat conversation IDs are deliberately restricted before they are
/// accepted from untrusted email headers or API requests.
pub fn valid_conversation_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn normalize_message_id(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
        .to_string()
}

fn parse_message_ids(raw: &str) -> Vec<String> {
    raw.split_whitespace()
        .map(normalize_message_id)
        .filter(|id| !id.is_empty())
        .collect()
}

/// Split an address list and extract lowercase addr-spec values.
fn split_addresses(field: &str) -> Vec<String> {
    if field.is_empty() {
        return vec![];
    }

    // Addr-spec extraction still works for the common display-name form. A
    // quoted comma is protected here so it does not split the display name.
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for ch in field.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if quoted => {
                current.push(ch);
                escaped = true;
            }
            '"' => {
                quoted = !quoted;
                current.push(ch);
            }
            ',' if !quoted => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }

    parts
        .iter()
        .map(|part| extract_first_address(part))
        .filter(|addr| !addr.is_empty())
        .collect()
}

fn extract_first_address(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(start) = raw.rfind('<') {
        if let Some(end) = raw[start + 1..].find('>') {
            return raw[start + 1..start + 1 + end]
                .trim()
                .to_lowercase();
        }
    }
    raw.to_lowercase()
}

/// Extract plain text recursively so common multipart/alternative and mixed
/// messages do not lose their body merely because of another nesting level.
fn extract_body(parsed: &ParsedMail) -> String {
    if parsed.subparts.is_empty() {
        return parsed.get_body().unwrap_or_else(|e| {
            tracing::warn!("Failed to decode email body: {e}");
            String::new()
        });
    }

    for part in &parsed.subparts {
        let content_type = part.ctype.mimetype.to_lowercase();
        if content_type == "text/plain" {
            let body = extract_body(part);
            if !body.is_empty() {
                return body;
            }
        }
    }

    for part in &parsed.subparts {
        if part.ctype.mimetype.to_lowercase().starts_with("text/") {
            let body = extract_body(part);
            if !body.is_empty() {
                return body;
            }
        }
    }

    String::new()
}

/// Extract and remove RoundChat WebDAV-linked attachment metadata:
/// `[file: filename | https://… | mime/type]`.
fn extract_attachments(body: &str) -> (String, Vec<Attachment>) {
    let mut attachments = Vec::new();
    let mut clean_lines = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();
        let parsed = if trimmed.starts_with("[file:") && trimmed.ends_with(']') {
            let inner = trimmed[6..trimmed.len() - 1].trim();
            let parts: Vec<&str> = inner.splitn(3, '|').collect();
            if parts.len() == 3 {
                let url = parts[1].trim();
                if url.starts_with("https://") || url.starts_with("http://") {
                    Some(Attachment {
                        filename: parts[0].trim().to_string(),
                        url: url.to_string(),
                        mime_type: parts[2].trim().to_string(),
                        size: -1,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(attachment) = parsed {
            attachments.push(attachment);
        } else {
            clean_lines.push(line);
        }
    }

    (clean_lines.join("\n").trim().to_string(), attachments)
}

fn parse_date(date_str: &str) -> String {
    if let Ok(dt) = mailparse::dateparse(date_str) {
        let datetime = chrono::DateTime::from_timestamp(dt, 0)
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
        return datetime.to_rfc3339();
    }
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_roundchat_threading_and_attachments() {
        let raw = b"From: Alice <alice@example.com>\r\nTo: bob@example.com\r\nSubject: Weekend\r\nMessage-ID: <child@example.com>\r\nIn-Reply-To: <parent@example.com>\r\nReferences: <root@example.com> <parent@example.com>\r\nX-RoundChat-Conversation-ID: rc_1234abcd\r\nDate: Wed, 01 Jan 2025 10:00:00 +0000\r\nContent-Type: text/plain\r\n\r\nSee this file\r\n[file: plan.pdf | https://files.example/plan.pdf | application/pdf]\r\n";
        let message = parse_raw_message(raw, "fallback").unwrap();

        assert_eq!(message.id, "child@example.com");
        assert_eq!(message.conversation_id.as_deref(), Some("rc_1234abcd"));
        assert_eq!(message.in_reply_to.as_deref(), Some("parent@example.com"));
        assert_eq!(message.references.len(), 2);
        assert_eq!(message.body, "See this file");
        assert_eq!(message.attachments.len(), 1);
        assert_eq!(message.attachments[0].filename, "plan.pdf");
    }

    #[test]
    fn finds_plain_text_in_nested_mime() {
        let raw = b"From: a@example.com\r\nTo: b@example.com\r\nContent-Type: multipart/mixed; boundary=outer\r\n\r\n--outer\r\nContent-Type: multipart/alternative; boundary=inner\r\n\r\n--inner\r\nContent-Type: text/plain\r\n\r\nhello\r\n--inner--\r\n--outer--\r\n";
        let message = parse_raw_message(raw, "fallback").unwrap();
        assert_eq!(message.body, "hello");
    }

    #[test]
    fn rejects_unsafe_conversation_ids() {
        assert!(valid_conversation_id("rc_0123-ABCD"));
        assert!(!valid_conversation_id("bad\r\nX-Injected: yes"));
        assert!(!valid_conversation_id(""));
    }
}
