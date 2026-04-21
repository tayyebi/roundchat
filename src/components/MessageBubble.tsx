/**
 * MessageBubble.
 *
 * Renders a single chat message in the iOS-style bubble format.
 * Own messages appear on the right in blue; incoming messages on the left in white.
 */
import React from 'react';
import { StyleSheet, Text, View } from 'react-native';
import type { Message } from '../models/Message';

interface Props {
  message: Message;
  isOwn: boolean;
}

function formatTime(dateStr: string): string {
  const d = new Date(dateStr);
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

export function MessageBubble({ message, isOwn }: Props) {
  return (
    <View style={[styles.row, isOwn ? styles.rowOwn : styles.rowOther]}>
      <View style={[styles.bubble, isOwn ? styles.bubbleOwn : styles.bubbleOther]}>
        {!isOwn && (
          <Text style={styles.sender}>{message.from}</Text>
        )}
        <Text style={[styles.body, isOwn ? styles.bodyOwn : styles.bodyOther]}>
          {message.body}
        </Text>
        {message.attachments.length > 0 && (
          <View style={styles.attachments}>
            {message.attachments.map((att) => (
              <Text key={att.url} style={styles.attachmentLink} numberOfLines={1}>
                📎 {att.filename}
              </Text>
            ))}
          </View>
        )}
        <Text style={[styles.time, isOwn ? styles.timeOwn : styles.timeOther]}>
          {formatTime(message.date)}
        </Text>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  row: { marginVertical: 2, flexDirection: 'row' },
  rowOwn: { justifyContent: 'flex-end' },
  rowOther: { justifyContent: 'flex-start' },
  bubble: {
    maxWidth: '75%',
    borderRadius: 18,
    paddingHorizontal: 14,
    paddingVertical: 10,
    shadowColor: '#000',
    shadowOpacity: 0.05,
    shadowOffset: { width: 0, height: 1 },
    shadowRadius: 2,
    elevation: 1,
  },
  bubbleOwn: { backgroundColor: '#007AFF', borderBottomRightRadius: 4 },
  bubbleOther: { backgroundColor: '#fff', borderBottomLeftRadius: 4 },
  sender: { fontSize: 11, fontWeight: '600', color: '#6C6C70', marginBottom: 2 },
  body: { fontSize: 15, lineHeight: 20 },
  bodyOwn: { color: '#fff' },
  bodyOther: { color: '#1C1C1E' },
  attachments: { marginTop: 4 },
  attachmentLink: { fontSize: 13, color: '#007AFF', textDecorationLine: 'underline' },
  time: { fontSize: 11, marginTop: 4, alignSelf: 'flex-end' },
  timeOwn: { color: 'rgba(255,255,255,0.7)' },
  timeOther: { color: '#8E8E93' },
});
