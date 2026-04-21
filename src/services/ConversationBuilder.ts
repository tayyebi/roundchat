/**
 * ConversationBuilder.
 *
 * Groups a flat list of Message objects into Conversation threads.
 * One responsibility: message grouping logic only.
 */
import type { Message } from '../models/Message';
import type { Conversation } from '../models/Conversation';

/** Derive a stable conversation key from the participant set and group name. */
function conversationKey(participants: string[], groupName: string | null): string {
  const sorted = [...participants].sort().join(',');
  return groupName ? `${groupName}:${sorted}` : sorted;
}

/** Collect all unique participant addresses from a set of messages. */
function collectParticipants(messages: Message[], localEmail: string): string[] {
  const addresses = new Set<string>();
  addresses.add(localEmail.toLowerCase());
  for (const msg of messages) {
    addresses.add(msg.from.toLowerCase());
    msg.to.forEach((a) => addresses.add(a.toLowerCase()));
  }
  return Array.from(addresses);
}

/**
 * Build conversations from a flat array of messages.
 *
 * Messages that share exactly the same participant set are grouped into the
 * same conversation.  For group chats (>2 total participants), a groupName
 * is derived from the email Subject (passed via the `subjectMap`).
 */
export function buildConversations(
  messages: Message[],
  localEmail: string,
  subjectMap: Map<string, string>,
): Conversation[] {
  const buckets = new Map<string, Message[]>();

  for (const msg of messages) {
    const allAddr = new Set([msg.from.toLowerCase(), ...msg.to.map((a) => a.toLowerCase())]);
    const participants = Array.from(allAddr);
    const others = participants.filter((p) => p !== localEmail.toLowerCase());

    // Use subjectMap for group name if there are multiple other participants.
    const groupName = others.length > 1 ? (subjectMap.get(msg.id) ?? null) : null;
    const key = conversationKey(participants, groupName);

    if (!buckets.has(key)) {
      buckets.set(key, []);
    }
    buckets.get(key)!.push(msg);
  }

  const conversations: Conversation[] = [];
  let seqId = 0;

  for (const [, msgs] of buckets) {
    seqId += 1;
    const sorted = [...msgs].sort((a, b) => new Date(a.date).getTime() - new Date(b.date).getTime());
    const participants = collectParticipants(sorted, localEmail);
    const others = participants.filter((p) => p !== localEmail.toLowerCase());
    const groupName = others.length > 1 ? (subjectMap.get(sorted[0].id) ?? null) : null;
    const unreadCount = sorted.filter((m) => !m.read).length;

    conversations.push({
      id: String(seqId),
      groupName,
      participants,
      messages: sorted,
      lastMessage: sorted[sorted.length - 1] ?? null,
      unreadCount,
    });
  }

  // Most recent conversation first.
  return conversations.sort((a, b) => {
    const aTime = a.lastMessage ? new Date(a.lastMessage.date).getTime() : 0;
    const bTime = b.lastMessage ? new Date(b.lastMessage.date).getTime() : 0;
    return bTime - aTime;
  });
}
