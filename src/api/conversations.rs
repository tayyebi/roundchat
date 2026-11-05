/// Conversation API handlers.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::api::auth::authenticated_session;
use crate::email::{conversation, mail_service, parser::valid_conversation_id, smtp};
use crate::models::{Attachment, Message};
use crate::state::AppState;
use crate::sync;

/// GET /api/conversations
pub async fn list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(session) = authenticated_session(&state, &headers).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "not logged in" })),
        )
            .into_response();
    };

    match sync::refresh_conversations(&state, &session.email, false).await {
        Ok(conversations) => Json(conversations).into_response(),
        Err(error) => {
            // Recent chat remains available during a transient mail outage.
            if let Ok(cached) = sync::cached_conversations(&state, &session.email).await {
                if !cached.is_empty() {
                    tracing::warn!("Serving cached conversations after sync failure: {error}");
                    return Json(cached).into_response();
                }
            }
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": error.to_string() })),
            )
                .into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct SendRequest {
    pub to: Vec<String>,
    pub body: String,
    pub group_name: Option<String>,
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

fn bad_request(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}

fn validate_attachment(attachment: &Attachment) -> Result<(), &'static str> {
    if attachment.filename.trim().is_empty()
        || attachment.filename.contains(|ch| ch == '\r' || ch == '\n' || ch == '|')
        || attachment.filename.len() > 255
    {
        return Err("invalid attachment filename");
    }
    if !(attachment.url.starts_with("https://") || attachment.url.starts_with("http://"))
        || attachment.url.chars().any(char::is_whitespace)
        || attachment.url.len() > 4096
    {
        return Err("attachment URLs must be valid HTTP(S) URLs");
    }
    if attachment.mime_type.trim().is_empty()
        || attachment.mime_type.contains(|ch| ch == '\r' || ch == '\n' || ch == '|')
        || attachment.mime_type.len() > 255
    {
        return Err("invalid attachment content type");
    }
    Ok(())
}

fn reply_subject(base: Option<String>, is_reply: bool, group: bool) -> String {
    let base = base.unwrap_or_else(|| {
        if group {
            "RoundChat group".to_string()
        } else {
            "RoundChat chat".to_string()
        }
    });
    if is_reply {
        format!("Re: {base}")
    } else {
        base
    }
}

fn wire_body(message: &Message) -> String {
    let mut body = message.body.clone();
    for attachment in &message.attachments {
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        body.push_str(&format!(
            "[file: {} | {} | {}]",
            attachment.filename.trim(),
            attachment.url.trim(),
            attachment.mime_type.trim()
        ));
    }
    body
}

/// POST /api/send
pub async fn send(State(state): State<AppState>, headers: HeaderMap, Json(request): Json<SendRequest>) -> Response {
    let Some(session) = authenticated_session(&state, &headers).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "not logged in" })),
        )
            .into_response();
    };

    let mut recipients: Vec<String> = request
        .to
        .iter()
        .map(|address| address.trim().to_lowercase())
        .filter(|address| !address.is_empty() && address != &session.email)
        .collect();
    recipients.sort();
    recipients.dedup();
    if recipients.is_empty() {
        return bad_request("at least one other recipient is required");
    }
    if recipients.len() > 100 || request.body.len() > 1_000_000 {
        return bad_request("message or recipient list is too large");
    }
    if request.body.trim().is_empty() && request.attachments.is_empty() {
        return bad_request("a message or attachment is required");
    }
    if request.attachments.len() > 20 {
        return bad_request("a maximum of 20 attachments is allowed");
    }
    for attachment in &request.attachments {
        if let Err(error) = validate_attachment(attachment) {
            return bad_request(error);
        }
    }
    if let Some(id) = &request.conversation_id {
        if !valid_conversation_id(id) {
            return bad_request("invalid conversation ID");
        }
    }

    let group_name = request
        .group_name
        .as_deref()
        .and_then(conversation::normalize_subject);
    let group_name = if recipients.len() > 1 && group_name.is_none() {
        Some("RoundChat group".to_string())
    } else {
        group_name
    };
    let derived_id = conversation::new_conversation_id(
        &session.email,
        &recipients,
        group_name.as_deref(),
    );
    let conversation_id = request.conversation_id.unwrap_or(derived_id);

    let cached = sync::cached_conversations(&state, &session.email)
        .await
        .unwrap_or_default();
    let existing = cached
        .iter()
        .find(|conversation| conversation.id == conversation_id);
    let reply = existing.and_then(|conversation| conversation.last_message.as_ref());
    let base_subject = group_name
        .or_else(|| existing.and_then(|conversation| conversation.subject.clone()));
    let subject = reply_subject(base_subject, reply.is_some(), recipients.len() > 1);

    let mut references = reply
        .map(|message| message.references.clone())
        .unwrap_or_default();
    if let Some(reply) = reply {
        if references.is_empty() {
            if let Some(parent) = &reply.in_reply_to {
                references.push(parent.clone());
            }
        }
        if !reply.id.is_empty() && !references.contains(&reply.id) {
            references.push(reply.id.clone());
        }
    }
    if references.len() > 50 {
        references = references.split_off(references.len() - 50);
    }
    let in_reply_to = reply.map(|message| message.id.clone());
    let message_id = format!("{}@roundchat.local", uuid::Uuid::new_v4());
    let message = Message {
        id: message_id.clone(),
        uid: None,
        source: String::new(),
        from: session.email.clone(),
        to: recipients.clone(),
        date: chrono::Utc::now().to_rfc3339(),
        body: request.body.trim().to_string(),
        attachments: request.attachments,
        read: true,
        subject: subject.clone(),
        conversation_id: Some(conversation_id.clone()),
        in_reply_to: in_reply_to.clone(),
        references: references.clone(),
    };

    let outbound = smtp::OutboundMessage {
        from: session.email.clone(),
        to: recipients,
        subject,
        body: wire_body(&message),
        message_id,
        conversation_id,
        in_reply_to,
        references,
    };
    if let Err(error) = mail_service::send_message(
        &state.config,
        &session.email,
        &session.password,
        outbound,
    )
    .await
    {
        return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": error.to_string() })),
        )
            .into_response();
    }

    match sync::record_outgoing(&state, &session.email, message).await {
        Ok(conversation) => Json(conversation).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("message was sent, but local history could not be updated: {error}")
            })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct MarkReadRequest {
    pub uids: Vec<u32>,
}

/// POST /api/mark-read
pub async fn mark_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<MarkReadRequest>,
) -> Response {
    let Some(session) = authenticated_session(&state, &headers).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    if request.uids.len() > 10_000 {
        return bad_request("too many message UIDs");
    }
    match sync::mark_messages_read(&state, &session.email, &request.uids).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(error) => {
            tracing::warn!("mark_read failed: {error}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": error.to_string() })),
            )
                .into_response()
        }
    }
}
