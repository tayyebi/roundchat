/**
 * IMAP service.
 *
 * Handles connecting to an IMAP server, listing messages, fetching bodies,
 * marking messages as read, and maintaining an IDLE connection for real-time
 * push delivery.  Each public method is focused on exactly one responsibility.
 *
 * The low-level TCP/TLS layer is provided by react-native-tcp-socket so that
 * the app compiles to a single native binary with no external proxy.
 */
import TcpSocket from 'react-native-tcp-socket';
import type { ImapConfig } from '../config/env';
import type { Message } from '../models/Message';
import { parseRawMessage } from './MessageParser';

type Socket = ReturnType<typeof TcpSocket.createConnection>;

interface PendingCommand {
  tag: string;
  resolve: (lines: string[]) => void;
  reject: (err: Error) => void;
  accum: string[];
}

let _tagSeq = 0;
function nextTag(): string {
  _tagSeq += 1;
  return `A${String(_tagSeq).padStart(4, '0')}`;
}

export class ImapService {
  private config: ImapConfig;
  private credentials: { email: string; password: string } | null = null;
  private socket: Socket | null = null;
  private buffer = '';
  private pending: PendingCommand | null = null;
  private idleCallback: ((uids: string[]) => void) | null = null;
  private idling = false;
  private idleTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(config: ImapConfig) {
    this.config = config;
  }

  /** Store credentials for later authentication. */
  setCredentials(email: string, password: string): void {
    this.credentials = { email, password };
  }

  /** Open the TCP/TLS connection and return after the server greeting. */
  async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const options = {
        host: this.config.host,
        port: this.config.port,
        tls: this.config.tls,
        tlsCheckValidity: true,
      };

      this.socket = TcpSocket.connectTLS(options, () => {
        // Connected – wait for greeting handled in onData
      });

      this.socket.on('data', (data: Buffer | string) => {
        const text = typeof data === 'string' ? data : data.toString('utf8');
        this.onData(text, resolve, reject);
      });

      this.socket.on('error', (err: Error) => {
        if (!this.pending) reject(err);
        else this.pending.reject(err);
      });
    });
  }

  private onData(chunk: string, connectResolve?: () => void, connectReject?: (e: Error) => void): void {
    this.buffer += chunk;
    const lines = this.buffer.split('\r\n');
    this.buffer = lines.pop() ?? '';

    for (const line of lines) {
      if (connectResolve && line.startsWith('* OK')) {
        connectResolve();
        connectResolve = undefined;
        continue;
      }

      if (connectResolve && line.startsWith('* BYE')) {
        connectReject?.(new Error(`Server rejected connection: ${line}`));
        continue;
      }

      if (this.idling && line === '+ idling') {
        continue;
      }

      if (this.idling && line.startsWith('* ') && line.includes('EXISTS')) {
        const match = line.match(/\* (\d+) EXISTS/);
        if (match && this.idleCallback) {
          this.idleCallback([match[1]]);
        }
        continue;
      }

      if (this.pending) {
        this.pending.accum.push(line);
        if (line.startsWith(this.pending.tag + ' OK') || line.startsWith(this.pending.tag + ' NO') || line.startsWith(this.pending.tag + ' BAD')) {
          const { resolve, reject, accum, tag } = this.pending;
          this.pending = null;
          if (line.startsWith(tag + ' OK')) resolve(accum);
          else reject(new Error(line));
        }
      }
    }
  }

  /** Send a tagged IMAP command and return the server response lines. */
  private sendCommand(command: string): Promise<string[]> {
    return new Promise((resolve, reject) => {
      const tag = nextTag();
      this.pending = { tag, resolve, reject, accum: [] };
      this.socket?.write(`${tag} ${command}\r\n`);
    });
  }

  /** Authenticate with LOGIN. */
  async login(): Promise<void> {
    if (!this.credentials) throw new Error('No credentials set');
    const { email, password } = this.credentials;
    await this.sendCommand(`LOGIN "${email}" "${password}"`);
  }

  /** Select the INBOX mailbox. */
  async selectInbox(): Promise<void> {
    await this.sendCommand('SELECT INBOX');
  }

  /** Fetch UIDs of all messages in INBOX. */
  async fetchAllUids(): Promise<string[]> {
    const lines = await this.sendCommand('UID SEARCH ALL');
    const searchLine = lines.find((l) => l.startsWith('* SEARCH'));
    if (!searchLine) return [];
    return searchLine.replace('* SEARCH', '').trim().split(' ').filter(Boolean);
  }

  /** Fetch the raw RFC 5322 text for a single UID. */
  async fetchRaw(uid: string): Promise<string> {
    const lines = await this.sendCommand(`UID FETCH ${uid} (BODY[])`);
    // Strip IMAP wrapper lines; concatenate the body section.
    const start = lines.findIndex((l) => l.includes('BODY[]'));
    if (start === -1) return '';
    const end = lines.findIndex((l, i) => i > start && l === ')');
    return lines.slice(start + 1, end === -1 ? undefined : end).join('\r\n');
  }

  /** Fetch and parse messages for a list of UIDs. */
  async fetchMessages(uids: string[]): Promise<Message[]> {
    const messages: Message[] = [];
    for (const uid of uids) {
      const raw = await this.fetchRaw(uid);
      const msg = parseRawMessage(raw, uid);
      if (msg) messages.push(msg);
    }
    return messages;
  }

  /** Mark a UID as read (\Seen flag). */
  async markRead(uid: string): Promise<void> {
    await this.sendCommand(`UID STORE ${uid} +FLAGS (\\Seen)`);
  }

  /**
   * Start IMAP IDLE.  `callback` is invoked whenever the server sends an
   * EXISTS notification, carrying the new message count as a string array.
   * Re-issues IDLE every 28 minutes to stay within the server's timeout.
   */
  async startIdle(callback: (uids: string[]) => void): Promise<void> {
    this.idleCallback = callback;
    this.idling = true;
    this.socket?.write('IDLE\r\n');
    this.scheduleIdleRefresh();
  }

  private scheduleIdleRefresh(): void {
    this.idleTimer = setTimeout(async () => {
      await this.doneIdle();
      this.socket?.write('IDLE\r\n');
      this.scheduleIdleRefresh();
    }, 28 * 60 * 1000);
  }

  /** Send DONE to exit IDLE mode. */
  async doneIdle(): Promise<void> {
    this.idling = false;
    return new Promise((resolve) => {
      this.socket?.write('DONE\r\n');
      setTimeout(resolve, 200);
    });
  }

  /** Gracefully log out and close the socket. */
  async disconnect(): Promise<void> {
    if (this.idleTimer) clearTimeout(this.idleTimer);
    if (this.idling) await this.doneIdle();
    try {
      await this.sendCommand('LOGOUT');
    } finally {
      this.socket?.destroy();
      this.socket = null;
    }
  }
}
