/// ConversationBuilder.
///
/// Groups a flat list of Message objects into Conversation threads keyed on
/// the sorted participant set.  Group chats (>1 other participant) carry a
/// group_name derived from the email Subject stored in a lookup map.

use std::collections::HashMap;
use crate::models::{Conversation, Message};

/// Build a stable key from the sorted participant list and optional group name.
fn conversation_key(participants: &[String], group_name: Option<&str>) -> String {
    let mut sorted = participants.to_vec();
    sorted.sort();
    match group_name {
        Some(g) if !g.is_empty() => format!("{}:{}", g, sorted.join(",")),
        _ => sorted.join(","),
    }
}

/// Collect the unique set of participant addresses from a slice of messages.
fn collect_participants(messages: &[Message], local_email: &str) -> Vec<String> {
    let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    set.insert(local_email.to_lowercase());
    for msg in messages {
        set.insert(msg.from.clone());
        for addr in &msg.to {
            set.insert(addr.clone());
        }
    }
    set.into_iter().collect()
}

/// Build conversations from a flat message list.
///
/// `subject_map` maps message id → email subject so that group threads can be
/// named after the subject without keeping it in the Message struct (which
/// intentionally omits subjects to maintain the chat illusion).
pub fn build_conversations(
    messages: Vec<Message>,
    local_email: &str,
    subject_map: &HashMap<String, String>,
) -> Vec<Conversation> {
    let lower_local = local_email.to_lowercase();

    // --- Bucket messages by conversation key ---
    let mut buckets: HashMap<String, Vec<Message>> = HashMap::new();

    for msg in messages {
        let others: Vec<_> = {
            let mut all = vec![msg.from.clone()];
            all.extend(msg.to.clone());
            all.into_iter()
                .filter(|a| *a != lower_local)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect()
        };
        let group_name = if others.len() > 1 {
            subject_map.get(&msg.id).filter(|s| !s.is_empty()).map(|s| s.as_str())
        } else {
            None
        };

        let participants: Vec<String> = {
            let mut all: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            all.insert(lower_local.clone());
            all.insert(msg.from.clone());
            for a in &msg.to {
                all.insert(a.clone());
            }
            all.into_iter().collect()
        };

        let key = conversation_key(&participants, group_name);
        buckets.entry(key).or_default().push(msg);
    }

    // --- Build Conversation structs from buckets ---
    let mut conversations: Vec<Conversation> = buckets
        .into_iter()
        .enumerate()
        .map(|(idx, (_, mut msgs))| {
            // Sort messages oldest-first.
            msgs.sort_by(|a, b| a.date.cmp(&b.date));

            let participants = collect_participants(&msgs, &lower_local);
            let others: Vec<_> = participants
                .iter()
                .filter(|a| *a != &lower_local)
                .collect();
            let group_name = if others.len() > 1 {
                subject_map
                    .get(&msgs[0].id)
                    .filter(|s| !s.is_empty())
                    .cloned()
            } else {
                None
            };

            let unread_count = msgs.iter().filter(|m| !m.read).count();
            let last_message = msgs.last().cloned();
            let id = format!("{}", idx + 1);

            Conversation {
                id,
                group_name,
                participants,
                messages: msgs,
                last_message,
                unread_count,
            }
        })
        .collect();

    // Sort most-recent-first.
    conversations.sort_by(|a, b| {
        let ta = a.last_message.as_ref().map(|m| m.date.as_str()).unwrap_or("");
        let tb = b.last_message.as_ref().map(|m| m.date.as_str()).unwrap_or("");
        tb.cmp(ta)
    });

    conversations
}
