import type { Attachment } from './Attachment';

/** A single chat message, derived from one email. */
export interface Message {
  /** Globally unique identifier (email Message-ID header, stripped of < >). */
  id: string;
  /** Sender email address. */
  from: string;
  /** Recipient email addresses (direct + CC). */
  to: string[];
  /** ISO-8601 date string. */
  date: string;
  /** Plain-text body. */
  body: string;
  /** File attachments, if any. */
  attachments: Attachment[];
  /** Whether the local user has read this message. */
  read: boolean;
}
