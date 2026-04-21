/**
 * POP3 service.
 *
 * Used when IMAP_HOST is not configured.  Connects to a POP3 server, lists
 * and downloads messages, then disconnects.  Periodic polling is orchestrated
 * by MailService; this class only handles a single fetch session.
 */
import TcpSocket from 'react-native-tcp-socket';
import type { Pop3Config } from '../config/env';
import type { Message } from '../models/Message';
import { parseRawMessage } from './MessageParser';

type Socket = ReturnType<typeof TcpSocket.createConnection>;

export class Pop3Service {
  private config: Pop3Config;
  private credentials: { email: string; password: string } | null = null;
  private socket: Socket | null = null;
  private buffer = '';

  constructor(config: Pop3Config) {
    this.config = config;
  }

  setCredentials(email: string, password: string): void {
    this.credentials = { email, password };
  }

  /** Open connection and wait for the +OK greeting. */
  async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const options = {
        host: this.config.host,
        port: this.config.port,
        tls: this.config.tls,
        tlsCheckValidity: true,
      };

      this.socket = TcpSocket.connectTLS(options, () => {});

      this.socket.on('data', (data: Buffer | string) => {
        const text = typeof data === 'string' ? data : data.toString('utf8');
        if (text.startsWith('+OK')) resolve();
        else if (text.startsWith('-ERR')) reject(new Error(text));
      });

      this.socket.on('error', reject);
    });
  }

  /** Send a single-line POP3 command and return the first response line. */
  private sendLine(command: string): Promise<string> {
    return new Promise((resolve, reject) => {
      const handler = (data: Buffer | string) => {
        this.buffer += typeof data === 'string' ? data : data.toString('utf8');
        const idx = this.buffer.indexOf('\r\n');
        if (idx !== -1) {
          const line = this.buffer.slice(0, idx);
          this.buffer = this.buffer.slice(idx + 2);
          this.socket?.off('data', handler);
          if (line.startsWith('+OK')) resolve(line);
          else reject(new Error(line));
        }
      };
      this.socket?.on('data', handler);
      this.socket?.write(`${command}\r\n`);
    });
  }

  /** Read a multi-line POP3 response (ends with ".\r\n"). */
  private readMultiLine(): Promise<string[]> {
    return new Promise((resolve) => {
      const handler = (data: Buffer | string) => {
        this.buffer += typeof data === 'string' ? data : data.toString('utf8');
        if (this.buffer.includes('\r\n.\r\n')) {
          const body = this.buffer.slice(0, this.buffer.indexOf('\r\n.\r\n'));
          this.buffer = '';
          this.socket?.off('data', handler);
          resolve(body.split('\r\n'));
        }
      };
      this.socket?.on('data', handler);
    });
  }

  /** Authenticate with USER/PASS. */
  async login(): Promise<void> {
    if (!this.credentials) throw new Error('No credentials set');
    const { email, password } = this.credentials;
    await this.sendLine(`USER ${email}`);
    await this.sendLine(`PASS ${password}`);
  }

  /** Return message count and total size. */
  async stat(): Promise<{ count: number; size: number }> {
    const line = await this.sendLine('STAT');
    const [, count, size] = line.split(' ');
    return { count: parseInt(count, 10), size: parseInt(size, 10) };
  }

  /** Fetch the raw text of message at the given 1-based index. */
  async fetchRaw(index: number): Promise<string> {
    await this.sendLine(`RETR ${index}`);
    const lines = await this.readMultiLine();
    return lines.join('\r\n');
  }

  /** Fetch and parse all messages on the server. */
  async fetchAllMessages(): Promise<Message[]> {
    const { count } = await this.stat();
    const messages: Message[] = [];
    for (let i = 1; i <= count; i++) {
      const raw = await this.fetchRaw(i);
      const msg = parseRawMessage(raw, String(i));
      if (msg) messages.push(msg);
    }
    return messages;
  }

  /** Send QUIT and close the socket. */
  async disconnect(): Promise<void> {
    try {
      await this.sendLine('QUIT');
    } finally {
      this.socket?.destroy();
      this.socket = null;
    }
  }
}
