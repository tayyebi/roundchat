/**
 * AppNavigator.
 *
 * Defines the complete navigation tree:
 *   - Unauthenticated: Login screen
 *   - Authenticated: Bottom tabs (Chats, Contacts, Files) + Chat detail stack
 */
import React from 'react';
import { NavigationContainer } from '@react-navigation/native';
import { createNativeStackNavigator } from '@react-navigation/native-stack';
import { createBottomTabNavigator } from '@react-navigation/bottom-tabs';
import { Ionicons } from '@expo/vector-icons';

import { LoginScreen } from '../screens/LoginScreen';
import { ConversationsScreen } from '../screens/ConversationsScreen';
import { ChatScreen } from '../screens/ChatScreen';
import { ContactsScreen } from '../screens/ContactsScreen';
import { FilesScreen } from '../screens/FilesScreen';
import { useAuth } from '../context/AuthContext';
import type { Conversation } from '../models/Conversation';

export type RootStackParams = {
  Login: undefined;
  Main: undefined;
  Chat: { conversation: Conversation };
};

export type MainTabParams = {
  Chats: undefined;
  Contacts: undefined;
  Files: undefined;
};

const Stack = createNativeStackNavigator<RootStackParams>();
const Tab = createBottomTabNavigator<MainTabParams>();

function MainTabs() {
  return (
    <Tab.Navigator
      screenOptions={({ route }) => ({
        headerShown: false,
        tabBarIcon: ({ color, size }) => {
          const icons: Record<string, string> = {
            Chats: 'chatbubbles',
            Contacts: 'people',
            Files: 'folder',
          };
          return <Ionicons name={icons[route.name] as any} size={size} color={color} />;
        },
        tabBarActiveTintColor: '#007AFF',
        tabBarInactiveTintColor: '#8E8E93',
        tabBarStyle: { borderTopWidth: 0.5, borderTopColor: '#C7C7CC' },
      })}
    >
      <Tab.Screen name="Chats" component={ConversationsScreen} />
      <Tab.Screen name="Contacts" component={ContactsScreen} />
      <Tab.Screen name="Files" component={FilesScreen} />
    </Tab.Navigator>
  );
}

export function AppNavigator() {
  const { session } = useAuth();

  return (
    <NavigationContainer>
      <Stack.Navigator screenOptions={{ headerShown: false }}>
        {session ? (
          <>
            <Stack.Screen name="Main" component={MainTabs} />
            <Stack.Screen name="Chat" component={ChatScreen} options={{ headerShown: true, title: '' }} />
          </>
        ) : (
          <Stack.Screen name="Login" component={LoginScreen} />
        )}
      </Stack.Navigator>
    </NavigationContainer>
  );
}
