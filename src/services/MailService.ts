/**
 * MailService — orchestrator.
 *
 * Decides at startup whether to use IMAP or POP3 based on the presence of
 * IMAP_HOST in the environment configuration, then delegates all protocol
 * work to the appropriate service class.  Exposes a unified async API so
 * that UI screens never need to know which protocol is active.
 */
import { getImapConfig, getPop3Config, getSmtpConfig } from '../config/env';
import { ImapService } from './ImapService';
import { Pop3Service } from './Pop3Service';
import { SmtpService, type OutboundMessage } from './SmtpService';
import { buildConversations } from './ConversationBuilder';
import type { Message } from '../models/Message';
import type { Conversation } from '../models/Conversation';

export class MailService {
  private email: string;
  private password: string;
  private imap: ImapService | null = null;
  private pop3: Pop3Service | null = null;
  private smtp: SmtpService;
  private pollTimer: ReturnType<typeof setInterval> | null = null;
  private subjectMap = new Map<string, string>();

  constructor(email: string, password: string) {
    this.email = email;
    this.password = password;

    const smtpConfig = getSmtpConfig();
    this.smtp = new SmtpService(smtpConfig);
    this.smtp.setCredentials(email, password);

    const imapConfig = getImapConfig();
    if (imapConfig) {
      this.imap = new ImapService(imapConfig);
      this.imap.setCredentials(email, password);
    } else {
      const pop3Config = getPop3Config();
      this.pop3 = new Pop3Service(pop3Config);
      this.pop3.setCredentials(email, password);
    }
  }

  /** Return true when IMAP is being used; false for POP3. */
  get isImap(): boolean {
    return this.imap !== null;
  }

  /** Connect and authenticate against the incoming mail server. */
  async connect(): Promise<void> {
    if (this.imap) {
      await this.imap.connect();
      await this.imap.login();
      await this.imap.selectInbox();
    } else if (this.pop3) {
      await this.pop3.connect();
      await this.pop3.login();
    }
  }

  /**
   * Fetch all messages and return them as grouped Conversation objects.
   * Stores the subject for each message so ConversationBuilder can name groups.
   */
  async fetchConversations(): Promise<Conversation[]> {
    const messages = await this.fetchMessages();
    return buildConversations(messages, this.email, this.subjectMap);
  }

  private async fetchMessages(): Promise<Message[]> {
    if (this.imap) {
      const uids = await this.imap.fetchAllUids();
      return this.imap.fetchMessages(uids);
    } else if (this.pop3) {
      return this.pop3.fetchAllMessages();
    }
    return [];
  }

  /**
   * Send a message.  The subject is set to the groupName for group threads;
   * for one-to-one chats an empty subject is used so the UI stays clean.
   */
  async sendMessage(to: string[], body: string, groupName?: string): Promise<void> {
    const outbound: OutboundMessage = {
      from: this.email,
      to,
      subject: groupName ?? '',
      body,
    };
    await this.smtp.sendMessage(outbound);
  }

  /**
   * Start real-time updates.
   * For IMAP: uses IDLE.  For POP3: starts a polling interval.
   * `onNewMessages` is called with the current full conversation list.
   */
  async startListening(onNewMessages: (conversations: Conversation[]) => void): Promise<void> {
    if (this.imap) {
      await this.imap.startIdle(async () => {
        const conversations = await this.fetchConversations();
        onNewMessages(conversations);
      });
    } else if (this.pop3) {
      const pop3Config = getPop3Config();
      this.pollTimer = setInterval(async () => {
        try {
          await this.pop3!.connect();
          await this.pop3!.login();
          const conversations = await this.fetchConversations();
          await this.pop3!.disconnect();
          onNewMessages(conversations);
        } catch {
          // Polling errors are non-fatal; the next interval will retry.
        }
      }, pop3Config.pollInterval * 1000);
    }
  }

  /** Stop listening for new messages. */
  async stopListening(): Promise<void> {
    if (this.imap) {
      await this.imap.doneIdle();
    }
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  }

  /** Mark a message as read on the server (IMAP only; no-op for POP3). */
  async markRead(uid: string): Promise<void> {
    if (this.imap) {
      await this.imap.markRead(uid);
    }
  }

  /** Disconnect from all servers. */
  async disconnect(): Promise<void> {
    await this.stopListening();
    if (this.imap) await this.imap.disconnect();
    if (this.pop3) await this.pop3.disconnect();
  }
}
