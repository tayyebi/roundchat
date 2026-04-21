import type { Message } from './Message';

/**
 * A conversation is an email thread presented as a chat.
 *
 * For one-to-one chats the `groupName` is null and the partner's address is
 * the sole entry in `participants` besides the local user.
 *
 * For group chats `groupName` holds the email Subject and `participants`
 * lists all addresses found in To/CC fields across the thread.
 */
export interface Conversation {
  /**
   * Stable thread identifier.  For IMAP this is the X-GM-THRID or the
   * In-Reply-To chain key; for POP3 it is a hash of participants + subject.
   */
  id: string;
  /** Null for one-to-one conversations. */
  groupName: string | null;
  /** All participant email addresses (including the local user). */
  participants: string[];
  /** Messages ordered oldest-first. */
  messages: Message[];
  /** Shortcut to the most recent message. */
  lastMessage: Message | null;
  /** Number of unread messages. */
  unreadCount: number;
}
