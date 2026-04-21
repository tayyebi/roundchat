/**
 * ContactAvatar.
 *
 * Shows either a remote image or a generated initials circle when no image
 * is available.  Keeps image loading isolated to this one component.
 */
import React from 'react';
import { Image, StyleSheet, Text, View } from 'react-native';

interface Props {
  name: string;
  url?: string;
  size?: number;
}

function initials(name: string): string {
  const parts = name.trim().split(/\s+/);
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
}

/** Deterministic pastel colour from a string. */
function colorFromName(name: string): string {
  const colors = ['#FF9500', '#34C759', '#007AFF', '#AF52DE', '#FF2D55', '#5AC8FA', '#FF6B00'];
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = name.charCodeAt(i) + ((hash << 5) - hash);
  return colors[Math.abs(hash) % colors.length];
}

export function ContactAvatar({ name, url, size = 44 }: Props) {
  const style = { width: size, height: size, borderRadius: size / 2 };

  if (url) {
    return <Image source={{ uri: url }} style={[styles.image, style]} />;
  }

  return (
    <View style={[styles.circle, style, { backgroundColor: colorFromName(name) }]}>
      <Text style={[styles.initials, { fontSize: size * 0.38 }]}>{initials(name)}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  image: { resizeMode: 'cover' },
  circle: { justifyContent: 'center', alignItems: 'center' },
  initials: { color: '#fff', fontWeight: '600' },
});
