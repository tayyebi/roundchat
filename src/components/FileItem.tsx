/**
 * FileItem.
 *
 * A single row in the Files screen.  Shows file icon, name, size, and a
 * delete button.
 */
import React from 'react';
import { StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import { Ionicons } from '@expo/vector-icons';
import type { RemoteFile } from '../services/WebDavService';

interface Props {
  file: RemoteFile;
  onDelete: () => void;
}

function humanSize(bytes: number): string {
  if (bytes < 0) return '—';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function iconForMime(mime: string): string {
  if (mime.startsWith('image/')) return 'image-outline';
  if (mime.startsWith('video/')) return 'videocam-outline';
  if (mime.includes('pdf')) return 'document-text-outline';
  if (mime.includes('zip') || mime.includes('compressed')) return 'archive-outline';
  return 'document-outline';
}

export function FileItem({ file, onDelete }: Props) {
  return (
    <View style={styles.container}>
      <Ionicons name={iconForMime(file.mimeType) as any} size={28} color="#007AFF" />
      <View style={styles.info}>
        <Text style={styles.name} numberOfLines={1}>{file.name}</Text>
        <Text style={styles.meta}>{humanSize(file.size)}</Text>
      </View>
      <TouchableOpacity onPress={onDelete} hitSlop={{ top: 10, bottom: 10, left: 10, right: 10 }}>
        <Ionicons name="trash-outline" size={20} color="#FF3B30" />
      </TouchableOpacity>
    </View>
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
  info: { flex: 1 },
  name: { fontSize: 15, color: '#1C1C1E', fontWeight: '500' },
  meta: { fontSize: 13, color: '#8E8E93', marginTop: 2 },
});
