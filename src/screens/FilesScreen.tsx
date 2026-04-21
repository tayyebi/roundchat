/**
 * FilesScreen.
 *
 * Lists files stored in the user's WebDAV directory and allows uploading
 * new files.  Files shared inside a chat are stored here and linked in the
 * message body.
 */
import React, { useCallback, useEffect, useState } from 'react';
import {
  ActivityIndicator,
  Alert,
  FlatList,
  SafeAreaView,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from 'react-native';
import * as DocumentPicker from 'expo-document-picker';
import { Ionicons } from '@expo/vector-icons';
import { useAuth } from '../context/AuthContext';
import { WebDavService, type RemoteFile } from '../services/WebDavService';
import { FileItem } from '../components/FileItem';
import { getWebDavConfig } from '../config/env';

export function FilesScreen() {
  const { session } = useAuth();
  const [files, setFiles] = useState<RemoteFile[]>([]);
  const [loading, setLoading] = useState(true);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function buildService() {
    if (!session) throw new Error('Not logged in');
    const config = getWebDavConfig();
    const svc = new WebDavService(config);
    svc.setCredentials(session.email, session.password);
    return svc;
  }

  const loadFiles = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await buildService().listFiles();
      setFiles(result);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to load files');
    } finally {
      setLoading(false);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session]);

  async function handleUpload() {
    const result = await DocumentPicker.getDocumentAsync({ copyToCacheDirectory: true });
    if (result.canceled || result.assets.length === 0) return;

    const asset = result.assets[0];
    setUploading(true);
    try {
      await buildService().uploadFile(asset.uri, asset.name, asset.mimeType ?? 'application/octet-stream');
      await loadFiles();
    } catch (err: unknown) {
      Alert.alert('Upload failed', err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setUploading(false);
    }
  }

  async function handleDelete(file: RemoteFile) {
    Alert.alert('Delete file', `Delete "${file.name}"?`, [
      { text: 'Cancel', style: 'cancel' },
      {
        text: 'Delete',
        style: 'destructive',
        onPress: async () => {
          try {
            await buildService().deleteFile(file.url);
            setFiles((prev) => prev.filter((f) => f.url !== file.url));
          } catch (err: unknown) {
            Alert.alert('Error', err instanceof Error ? err.message : 'Unknown error');
          }
        },
      },
    ]);
  }

  useEffect(() => { loadFiles(); }, [loadFiles]);

  if (loading) {
    return (
      <SafeAreaView style={styles.center}>
        <ActivityIndicator size="large" color="#007AFF" />
      </SafeAreaView>
    );
  }

  return (
    <SafeAreaView style={styles.container}>
      <View style={styles.header}>
        <Text style={styles.title}>Files</Text>
        <TouchableOpacity style={styles.uploadButton} onPress={handleUpload} disabled={uploading}>
          {uploading ? (
            <ActivityIndicator color="#007AFF" size="small" />
          ) : (
            <Ionicons name="cloud-upload-outline" size={22} color="#007AFF" />
          )}
        </TouchableOpacity>
      </View>

      {error && (
        <View style={styles.errorBanner}>
          <Text style={styles.errorText}>{error}</Text>
        </View>
      )}

      <FlatList
        data={files}
        keyExtractor={(item) => item.url}
        renderItem={({ item }) => <FileItem file={item} onDelete={() => handleDelete(item)} />}
        ItemSeparatorComponent={() => <View style={styles.separator} />}
        ListEmptyComponent={
          <View style={styles.empty}>
            <Text style={styles.emptyText}>No files uploaded yet</Text>
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
  uploadButton: { padding: 4 },
  errorBanner: { backgroundColor: '#FFF3F3', padding: 12, borderBottomWidth: 0.5, borderBottomColor: '#FFCDD2' },
  errorText: { color: '#FF3B30', fontSize: 14 },
  separator: { height: 0.5, backgroundColor: '#C7C7CC' },
  empty: { alignItems: 'center', marginTop: 80 },
  emptyText: { color: '#6C6C70', fontSize: 16 },
});
