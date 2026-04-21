/**
 * ChatScreen.
 *
 * Displays the messages in a single conversation and allows sending replies.
 * The screen receives the full Conversation object from the navigator so it
 * can render immediately without an extra network fetch.
 */
import React, { useRef, useState } from 'react';
import {
  FlatList,
  KeyboardAvoidingView,
  Platform,
  SafeAreaView,
  StyleSheet,
  Text,
  TextInput,
  TouchableOpacity,
  View,
} from 'react-native';
import { useNavigation, useRoute } from '@react-navigation/native';
import type { NativeStackNavigationProp } from '@react-navigation/native-stack';
import type { RouteProp } from '@react-navigation/native';
import { Ionicons } from '@expo/vector-icons';
import { useAuth } from '../context/AuthContext';
import { MailService } from '../services/MailService';
import { MessageBubble } from '../components/MessageBubble';
import type { Message } from '../models/Message';
import type { RootStackParams } from '../navigation/AppNavigator';

type Nav = NativeStackNavigationProp<RootStackParams>;
type Route = RouteProp<RootStackParams, 'Chat'>;

export function ChatScreen() {
  const { session } = useAuth();
  const navigation = useNavigation<Nav>();
  const route = useRoute<Route>();
  const { conversation } = route.params;

  const [messages, setMessages] = useState<Message[]>(conversation.messages);
  const [text, setText] = useState('');
  const [sending, setSending] = useState(false);
  const listRef = useRef<FlatList>(null);

  const title = conversation.groupName
    ?? conversation.participants.filter((p: string) => p !== session?.email?.toLowerCase()).join(', ');

  React.useLayoutEffect(() => {
    navigation.setOptions({ title });
  }, [navigation, title]);

  async function handleSend() {
    if (!text.trim() || !session) return;
    setSending(true);
    try {
      const mail = new MailService(session.email, session.password);
      const recipients = conversation.participants.filter((p: string) => p !== session.email.toLowerCase());
      await mail.sendMessage(recipients, text.trim(), conversation.groupName ?? undefined);
      // Optimistically add the message to the UI.
      const optimistic: Message = {
        id: `local-${Date.now()}`,
        from: session.email,
        to: recipients,
        date: new Date().toISOString(),
        body: text.trim(),
        attachments: [],
        read: true,
      };
      setMessages((prev) => [...prev, optimistic]);
      setText('');
      setTimeout(() => listRef.current?.scrollToEnd({ animated: true }), 100);
    } finally {
      setSending(false);
    }
  }

  return (
    <SafeAreaView style={styles.container}>
      <KeyboardAvoidingView style={styles.flex} behavior={Platform.OS === 'ios' ? 'padding' : 'height'} keyboardVerticalOffset={90}>
        <FlatList
          ref={listRef}
          data={messages}
          keyExtractor={(item) => item.id}
          renderItem={({ item }) => (
            <MessageBubble message={item} isOwn={item.from === session?.email?.toLowerCase()} />
          )}
          contentContainerStyle={styles.listContent}
          onLayout={() => listRef.current?.scrollToEnd({ animated: false })}
        />

        <View style={styles.inputRow}>
          <TextInput
            style={styles.input}
            placeholder="Message"
            placeholderTextColor="#999"
            value={text}
            onChangeText={setText}
            multiline
            maxLength={4000}
          />
          <TouchableOpacity style={styles.sendButton} onPress={handleSend} disabled={sending || !text.trim()}>
            <Ionicons name="send" size={20} color={text.trim() ? '#007AFF' : '#C7C7CC'} />
          </TouchableOpacity>
        </View>
      </KeyboardAvoidingView>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#F2F2F7' },
  flex: { flex: 1 },
  listContent: { paddingHorizontal: 12, paddingVertical: 8 },
  inputRow: {
    flexDirection: 'row',
    alignItems: 'flex-end',
    paddingHorizontal: 12,
    paddingVertical: 8,
    backgroundColor: '#fff',
    borderTopWidth: 0.5,
    borderTopColor: '#C7C7CC',
    gap: 8,
  },
  input: {
    flex: 1,
    backgroundColor: '#F2F2F7',
    borderRadius: 20,
    paddingHorizontal: 14,
    paddingVertical: 10,
    fontSize: 15,
    maxHeight: 120,
    color: '#1C1C1E',
  },
  sendButton: {
    width: 36,
    height: 36,
    justifyContent: 'center',
    alignItems: 'center',
  },
});
