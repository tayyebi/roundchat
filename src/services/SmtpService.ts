/**
 * SMTP service.
 *
 * Sends outgoing messages via STARTTLS or implicit TLS (SMTPS).
 * Each send operation opens a fresh connection so that the app does not need
 * to manage a persistent authenticated session.
 */
import TcpSocket from 'react-native-tcp-socket';
import type { SmtpConfig } from '../config/env';

type Socket = ReturnType<typeof TcpSocket.createConnection>;

export interface OutboundMessage {
  from: string;
  to: string[];
  subject: string;
  body: string;
  /** Optional pre-encoded MIME parts to append (for attachments). */
  extraMimeParts?: string;
}

export class SmtpService {
  private config: SmtpConfig;
  private credentials: { email: string; password: string } | null = null;
  private socket: Socket | null = null;
  private buffer = '';

  constructor(config: SmtpConfig) {
    this.config = config;
  }

  setCredentials(email: string, password: string): void {
    this.credentials = { email, password };
  }

  private readLine(): Promise<string> {
    return new Promise((resolve) => {
      const handler = (data: Buffer | string) => {
        this.buffer += typeof data === 'string' ? data : data.toString('utf8');
        // SMTP responses end with <CRLF>; continuation lines have '-' at pos 3.
        const lines = this.buffer.split('\r\n');
        this.buffer = lines.pop() ?? '';
        for (const line of lines) {
          // Last line of a possibly multi-line reply has a space at position 3.
          if (line.length >= 4 && line[3] === ' ') {
            this.socket?.off('data', handler);
            resolve(line);
            return;
          }
        }
      };
      this.socket?.on('data', handler);
    });
  }

  private async send(data: string): Promise<string> {
    return new Promise((resolve) => {
      const handler = (chunk: Buffer | string) => {
        this.buffer += typeof chunk === 'string' ? chunk : chunk.toString('utf8');
        const lines = this.buffer.split('\r\n');
        this.buffer = lines.pop() ?? '';
        for (const line of lines) {
          if (line.length >= 4 && line[3] === ' ') {
            this.socket?.off('data', handler);
            resolve(line);
            return;
          }
        }
      };
      this.socket?.on('data', handler);
      this.socket?.write(data + '\r\n');
    });
  }

  private base64(str: string): string {
    return btoa(str);
  }

  /** Send the message and close the connection. */
  async sendMessage(msg: OutboundMessage): Promise<void> {
    if (!this.credentials) throw new Error('No credentials set');

    await this.connect();

    // Greeting already consumed by connect(); send EHLO with the sender's domain.
    const senderDomain = msg.from.split('@')[1] ?? 'localhost';
    await this.send(`EHLO ${senderDomain}`);

    // AUTH LOGIN
    await this.send('AUTH LOGIN');
    await this.send(this.base64(this.credentials.email));
    await this.send(this.base64(this.credentials.password));

    // Envelope
    await this.send(`MAIL FROM:<${msg.from}>`);
    for (const addr of msg.to) {
      await this.send(`RCPT TO:<${addr}>`);
    }

    // DATA
    await this.send('DATA');
    const raw = this.buildRaw(msg);
    // Send entire message body; end with \r\n.\r\n
    await new Promise<void>((resolve) => {
      const handler = (chunk: Buffer | string) => {
        const text = typeof chunk === 'string' ? chunk : chunk.toString('utf8');
        if (text.startsWith('250')) {
          this.socket?.off('data', handler);
          resolve();
        }
      };
      this.socket?.on('data', handler);
      this.socket?.write(raw + '\r\n.\r\n');
    });

    await this.send('QUIT');
    this.socket?.destroy();
    this.socket = null;
  }

  private buildRaw(msg: OutboundMessage): string {
    const date = new Date().toUTCString();
    const toHeader = msg.to.join(', ');
    const subject = msg.subject || '(no subject)';
    const body = msg.body;

    let raw =
      `Date: ${date}\r\n` +
      `From: ${msg.from}\r\n` +
      `To: ${toHeader}\r\n` +
      `Subject: ${subject}\r\n` +
      `MIME-Version: 1.0\r\n` +
      `Content-Type: text/plain; charset=UTF-8\r\n` +
      `Content-Transfer-Encoding: 8bit\r\n` +
      `\r\n` +
      body;

    if (msg.extraMimeParts) {
      raw += '\r\n' + msg.extraMimeParts;
    }

    return raw;
  }

  private connect(): Promise<void> {
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
        if (text.startsWith('220')) resolve();
        else if (text.startsWith('4') || text.startsWith('5')) reject(new Error(text));
      });

      this.socket.on('error', reject);
    });
  }
}
