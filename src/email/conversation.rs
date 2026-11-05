/// ConversationBuilder.
///
/// RoundChat messages share an explicit stable conversation ID. Ordinary
/// email is grouped with standard References/In-Reply-To headers when
/// available, with deterministic participant/subject keys as a fallback.

use std::collections::{BTreeSet, HashMap};

use crate::email::parser::valid_conversation_id;
use crate::models::{Conversation, Message};

/// Deterministic FNV-1a based IDs avoid the unstable HashMap enumeration IDs
/// used by the original implementation without adding another dependency.
fn stable_id(namespace: &str, value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{namespace}_{hash:016x}")
}

/// Remove common reply/forward prefixes for grouping and reply subjects.
pub fn normalize_subject(subject: &str) -> Option<String> {
    let mut value = subject.trim();
    loop {
        let lower = value.to_lowercase();
        let prefix_len = ["re:", "fw:", "fwd:"]
            .iter()
            .find(|prefix| lower.starts_with(**prefix))
            .map(|prefix| prefix.len());
        match prefix_len {
            Some(len) => value = value[len..].trim_start(),
            None => break,
        }
    }
    (!value.is_empty()).then(|| value.to_string())
}

fn participants_for(message: &Message, local_email: &str) -> Vec<String> {
    let mut participants = BTreeSet::new();
    participants.insert(local_email.to_lowercase());
    if !message.from.is_empty() {
        participants.insert(message.from.to_lowercase());
    }
    for address in &message.to {
        if !address.is_empty() {
            participants.insert(address.to_lowercase());
        }
    }
    participants.into_iter().collect()
}

fn group_name_for(message: &Message, participants: &[String], local_email: &str) -> Option<String> {
    let other_count = participants
        .iter()
        .filter(|address| *address != local_email)
        .count();
    (other_count > 1)
        .then(|| normalize_subject(&message.subject))
        .flatten()
}

fn derived_id(participants: &[String], group_name: Option<&str>) -> String {
    let participants = participants.join(",");
    match group_name {
        Some(group) => stable_id(
            "rc",
            &format!("group\n{}\n{participants}", group.to_lowercase()),
        ),
        None => stable_id("rc", &format!("direct\n{participants}")),
    }
}

/// Compute the ID used for a newly composed RoundChat conversation.
pub fn new_conversation_id(
    local_email: &str,
    recipients: &[String],
    group_name: Option<&str>,
) -> String {
    let local_email = local_email.to_lowercase();
    let mut participants = BTreeSet::new();
    participants.insert(local_email.clone());
    for recipient in recipients {
        participants.insert(recipient.to_lowercase());
    }
    let participants: Vec<String> = participants.into_iter().collect();
    let other_count = participants
        .iter()
        .filter(|address| *address != &local_email)
        .count();
    let group_name = (other_count > 1)
        .then(|| group_name.and_then(normalize_subject))
        .flatten();
    derived_id(&participants, group_name.as_deref())
}

/// Build stable conversations from a flat message list.
pub fn build_conversations(messages: Vec<Message>, local_email: &str) -> Vec<Conversation> {
    let lower_local = local_email.to_lowercase();
    let mut ordered = messages;
    ordered.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.id.cmp(&b.id)));

    let mut buckets: HashMap<String, Vec<Message>> = HashMap::new();
    let mut message_conversations: HashMap<String, String> = HashMap::new();

    for message in ordered {
        let participants = participants_for(&message, &lower_local);
        let group_name = group_name_for(&message, &participants, &lower_local);
        let fallback = derived_id(&participants, group_name.as_deref());

        let referenced_conversation = message
            .references
            .iter()
            .rev()
            .chain(message.in_reply_to.iter())
            .find_map(|id| message_conversations.get(id))
            .cloned();

        let id = if let Some(explicit) = message
            .conversation_id
            .as_ref()
            .filter(|id| valid_conversation_id(id))
        {
            explicit.clone()
        } else if let Some(referenced) = referenced_conversation {
            referenced
        } else if group_name.is_some() {
            // Every message in an ordinary email thread uses the root
            // Message-ID, including the root itself. This remains stable even
            // when a reply is processed before the root is available.
            let root = message
                .references
                .first()
                .or(message.in_reply_to.as_ref())
                .map(String::as_str)
                .unwrap_or(message.id.as_str());
            stable_id("em", root)
        } else {
            fallback
        };

        if !message.id.is_empty() {
            message_conversations.insert(message.id.clone(), id.clone());
        }
        buckets.entry(id).or_default().push(message);
    }

    let mut conversations: Vec<Conversation> = buckets
        .into_iter()
        .map(|(id, mut messages)| {
            messages.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.id.cmp(&b.id)));

            let mut participants = BTreeSet::new();
            participants.insert(lower_local.clone());
            let mut subject = None;
            for message in &messages {
                for participant in participants_for(message, &lower_local) {
                    participants.insert(participant);
                }
                if subject.is_none() {
                    subject = normalize_subject(&message.subject);
                }
            }
            let participants: Vec<String> = participants.into_iter().collect();
            let other_count = participants
                .iter()
                .filter(|address| *address != &lower_local)
                .count();
            let group_name = (other_count > 1).then(|| subject.clone()).flatten();
            let unread_count = messages
                .iter()
                .filter(|message| !message.read && message.from != lower_local)
                .count();
            let last_message = messages.last().cloned();

            Conversation {
                id,
                group_name,
                subject,
                participants,
                messages,
                last_message,
                unread_count,
            }
        })
        .collect();

    conversations.sort_by(|a, b| {
        let a_date = a.last_message.as_ref().map(|m| m.date.as_str()).unwrap_or("");
        let b_date = b.last_message.as_ref().map(|m| m.date.as_str()).unwrap_or("");
        b_date.cmp(a_date).then_with(|| a.id.cmp(&b.id))
    });
    conversations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: &str, from: &str, to: &[&str], subject: &str) -> Message {
        Message {
            id: id.to_string(),
            uid: None,
            source: String::new(),
            from: from.to_string(),
            to: to.iter().map(|value| value.to_string()).collect(),
            date: "2025-01-01T00:00:00+00:00".to_string(),
            body: "hello".to_string(),
            attachments: vec![],
            read: false,
            subject: subject.to_string(),
            conversation_id: None,
            in_reply_to: None,
            references: vec![],
        }
    }

    #[test]
    fn direct_conversation_ids_are_stable() {
        let first = message("one", "alice@example.com", &["me@example.com"], "First");
        let second = message("two", "me@example.com", &["alice@example.com"], "Unrelated");
        let conversations = build_conversations(vec![second, first], "me@example.com");

        assert_eq!(conversations.len(), 1);
        assert!(conversations[0].id.starts_with("rc_"));
        assert_eq!(
            conversations[0].id,
            new_conversation_id(
                "me@example.com",
                &["alice@example.com".to_string()],
                None,
            )
        );
    }

    #[test]
    fn standard_replies_join_an_existing_group_thread() {
        let root = message(
            "root",
            "alice@example.com",
            &["me@example.com", "bob@example.com"],
            "Plans",
        );
        let mut reply = message(
            "reply",
            "bob@example.com",
            &["me@example.com", "alice@example.com"],
            "Re: Plans",
        );
        reply.in_reply_to = Some("root".to_string());
        reply.references = vec!["root".to_string()];

        let conversations = build_conversations(vec![reply, root], "me@example.com");
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].messages.len(), 2);
        assert_eq!(conversations[0].group_name.as_deref(), Some("Plans"));
    }

    #[test]
    fn unread_count_excludes_own_messages() {
        let mut incoming = message("one", "alice@example.com", &["me@example.com"], "Hi");
        incoming.read = false;
        let outgoing = message("two", "me@example.com", &["alice@example.com"], "Hi");
        let conversations = build_conversations(vec![incoming, outgoing], "me@example.com");
        assert_eq!(conversations[0].unread_count, 1);
    }
}
