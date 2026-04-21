/**
 * ConversationItem.
 *
 * A single row in the conversations list.  Mimics the iOS Messages app style:
 * avatar, name/group on the left, timestamp on the right, and a snippet of the
 * last message below.
 */
import React from 'react';
import { StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import { ContactAvatar } from './ContactAvatar';
import type { Conversation } from '../models/Conversation';

interface Props {
  conversation: Conversation;
  localEmail: string;
  onPress: () => void;
}

function formatDate(dateStr: string): string {
  const d = new Date(dateStr);
  const now = new Date();
  const isToday = d.toDateString() === now.toDateString();
  if (isToday) return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  return d.toLocaleDateString([], { month: 'short', day: 'numeric' });
}

export function ConversationItem({ conversation, localEmail, onPress }: Props) {
  const others = conversation.participants.filter(
    (p) => p.toLowerCase() !== localEmail.toLowerCase(),
  );
  const displayName = conversation.groupName ?? (others.join(', ') || 'Unknown');
  const snippet = conversation.lastMessage?.body ?? '';
  const time = conversation.lastMessage ? formatDate(conversation.lastMessage.date) : '';

  return (
    <TouchableOpacity style={styles.container} onPress={onPress} activeOpacity={0.7}>
      <ContactAvatar name={displayName} />
      <View style={styles.content}>
        <View style={styles.row}>
          <Text style={[styles.name, conversation.unreadCount > 0 && styles.nameBold]} numberOfLines={1}>
            {displayName}
          </Text>
          <Text style={styles.time}>{time}</Text>
        </View>
        <View style={styles.row}>
          <Text style={[styles.snippet, conversation.unreadCount > 0 && styles.snippetBold]} numberOfLines={1}>
            {snippet}
          </Text>
          {conversation.unreadCount > 0 && (
            <View style={styles.badge}>
              <Text style={styles.badgeText}>{conversation.unreadCount}</Text>
            </View>
          )}
        </View>
      </View>
    </TouchableOpacity>
  );
}

const styles = StyleSheet.create({
  container: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 16,
    paddingVertical: 12,
    backgroundColor: '#fff',
    gap: 12,
  },
  content: { flex: 1 },
  row: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center' },
  name: { fontSize: 16, color: '#1C1C1E', flex: 1, marginRight: 8 },
  nameBold: { fontWeight: '600' },
  time: { fontSize: 13, color: '#8E8E93' },
  snippet: { fontSize: 14, color: '#8E8E93', flex: 1, marginRight: 8 },
  snippetBold: { color: '#1C1C1E', fontWeight: '500' },
  badge: {
    backgroundColor: '#007AFF',
    borderRadius: 10,
    minWidth: 20,
    height: 20,
    justifyContent: 'center',
    alignItems: 'center',
    paddingHorizontal: 6,
  },
  badgeText: { color: '#fff', fontSize: 12, fontWeight: '700' },
});
