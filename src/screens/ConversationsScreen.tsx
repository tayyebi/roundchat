/**
 * ConversationsScreen.
 *
 * Shows the list of chat conversations.  Connects to the mail server on
 * mount, fetches all threads, then starts listening for new messages.
 */
import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  ActivityIndicator,
  FlatList,
  SafeAreaView,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from 'react-native';
import { useNavigation } from '@react-navigation/native';
import type { NativeStackNavigationProp } from '@react-navigation/native-stack';
import { useAuth } from '../context/AuthContext';
import { MailService } from '../services/MailService';
import { ConversationItem } from '../components/ConversationItem';
import type { Conversation } from '../models/Conversation';
import type { RootStackParams } from '../navigation/AppNavigator';

type Nav = NativeStackNavigationProp<RootStackParams>;

export function ConversationsScreen() {
  const { session, logout } = useAuth();
  const navigation = useNavigation<Nav>();
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mailRef = useRef<MailService | null>(null);

  const loadConversations = useCallback(async () => {
    if (!session) return;
    try {
      if (!mailRef.current) {
        mailRef.current = new MailService(session.email, session.password);
        await mailRef.current.connect();
      }
      const convos = await mailRef.current.fetchConversations();
      setConversations(convos);
      setError(null);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to load messages');
    } finally {
      setLoading(false);
    }
  }, [session]);

  useEffect(() => {
    loadConversations().then(() => {
      if (mailRef.current) {
        mailRef.current.startListening((convos) => setConversations(convos));
      }
    });
    return () => {
      mailRef.current?.disconnect();
      mailRef.current = null;
    };
  }, [loadConversations]);

  function openChat(conversation: Conversation) {
    navigation.navigate('Chat', { conversation });
  }

  if (loading) {
    return (
      <SafeAreaView style={styles.center}>
        <ActivityIndicator size="large" color="#007AFF" />
        <Text style={styles.loadingText}>Loading chats…</Text>
      </SafeAreaView>
    );
  }

  if (error) {
    return (
      <SafeAreaView style={styles.center}>
        <Text style={styles.errorText}>{error}</Text>
        <TouchableOpacity style={styles.retryButton} onPress={loadConversations}>
          <Text style={styles.retryText}>Retry</Text>
        </TouchableOpacity>
      </SafeAreaView>
    );
  }

  return (
    <SafeAreaView style={styles.container}>
      <View style={styles.header}>
        <Text style={styles.title}>Chats</Text>
        <TouchableOpacity onPress={logout}>
          <Text style={styles.logoutText}>Sign out</Text>
        </TouchableOpacity>
      </View>

      <FlatList
        data={conversations}
        keyExtractor={(item) => item.id}
        renderItem={({ item }) => (
          <ConversationItem conversation={item} localEmail={session?.email ?? ''} onPress={() => openChat(item)} />
        )}
        ListEmptyComponent={
          <View style={styles.empty}>
            <Text style={styles.emptyText}>No conversations yet</Text>
          </View>
        }
      />
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#F2F2F7' },
  center: { flex: 1, justifyContent: 'center', alignItems: 'center', backgroundColor: '#F2F2F7' },
  header: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingHorizontal: 16,
    paddingVertical: 12,
    backgroundColor: '#fff',
    borderBottomWidth: 0.5,
    borderBottomColor: '#C7C7CC',
  },
  title: { fontSize: 28, fontWeight: '700', color: '#1C1C1E' },
  logoutText: { fontSize: 15, color: '#FF3B30' },
  loadingText: { marginTop: 12, color: '#6C6C70', fontSize: 15 },
  errorText: { color: '#FF3B30', fontSize: 15, textAlign: 'center', marginHorizontal: 24 },
  retryButton: { marginTop: 16, paddingHorizontal: 24, paddingVertical: 10, backgroundColor: '#007AFF', borderRadius: 10 },
  retryText: { color: '#fff', fontWeight: '600', fontSize: 15 },
  empty: { flex: 1, alignItems: 'center', marginTop: 80 },
  emptyText: { color: '#6C6C70', fontSize: 16 },
});
