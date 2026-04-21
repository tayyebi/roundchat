/**
 * ContactsScreen.
 *
 * Fetches and displays the user's address book from CardDAV.
 */
import React, { useCallback, useEffect, useState } from 'react';
import {
  ActivityIndicator,
  FlatList,
  SafeAreaView,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from 'react-native';
import { useAuth } from '../context/AuthContext';
import { CardDavService } from '../services/CardDavService';
import { ContactAvatar } from '../components/ContactAvatar';
import { getCardDavConfig } from '../config/env';
import type { Contact } from '../models/Contact';

export function ContactsScreen() {
  const { session } = useAuth();
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadContacts = useCallback(async () => {
    if (!session) return;
    setLoading(true);
    setError(null);
    try {
      const config = getCardDavConfig();
      const service = new CardDavService(config);
      service.setCredentials(session.email, session.password);
      const result = await service.fetchAllContacts();
      setContacts(result);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to load contacts');
    } finally {
      setLoading(false);
    }
  }, [session]);

  useEffect(() => { loadContacts(); }, [loadContacts]);

  if (loading) {
    return (
      <SafeAreaView style={styles.center}>
        <ActivityIndicator size="large" color="#007AFF" />
      </SafeAreaView>
    );
  }

  if (error) {
    return (
      <SafeAreaView style={styles.center}>
        <Text style={styles.errorText}>{error}</Text>
        <TouchableOpacity style={styles.retryButton} onPress={loadContacts}>
          <Text style={styles.retryText}>Retry</Text>
        </TouchableOpacity>
      </SafeAreaView>
    );
  }

  return (
    <SafeAreaView style={styles.container}>
      <View style={styles.header}>
        <Text style={styles.title}>Contacts</Text>
      </View>

      <FlatList
        data={contacts}
        keyExtractor={(item) => item.id}
        renderItem={({ item }) => (
          <View style={styles.contactRow}>
            <ContactAvatar name={item.displayName} url={item.avatarUrl} />
            <View style={styles.contactInfo}>
              <Text style={styles.contactName}>{item.displayName}</Text>
              <Text style={styles.contactEmail}>{item.email}</Text>
            </View>
          </View>
        )}
        ItemSeparatorComponent={() => <View style={styles.separator} />}
        ListEmptyComponent={
          <View style={styles.empty}>
            <Text style={styles.emptyText}>No contacts found</Text>
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
    paddingHorizontal: 16,
    paddingVertical: 12,
    backgroundColor: '#fff',
    borderBottomWidth: 0.5,
    borderBottomColor: '#C7C7CC',
  },
  title: { fontSize: 28, fontWeight: '700', color: '#1C1C1E' },
  contactRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 16,
    paddingVertical: 12,
    backgroundColor: '#fff',
    gap: 12,
  },
  contactInfo: { flex: 1 },
  contactName: { fontSize: 16, fontWeight: '500', color: '#1C1C1E' },
  contactEmail: { fontSize: 13, color: '#6C6C70', marginTop: 2 },
  separator: { height: 0.5, backgroundColor: '#C7C7CC', marginLeft: 72 },
  empty: { alignItems: 'center', marginTop: 80 },
  emptyText: { color: '#6C6C70', fontSize: 16 },
  errorText: { color: '#FF3B30', fontSize: 15, textAlign: 'center', marginHorizontal: 24 },
  retryButton: { marginTop: 16, paddingHorizontal: 24, paddingVertical: 10, backgroundColor: '#007AFF', borderRadius: 10 },
  retryText: { color: '#fff', fontWeight: '600', fontSize: 15 },
});
