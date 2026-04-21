import { parseRawMessage } from '../../src/services/MessageParser';

const SIMPLE_EMAIL = [
  'Message-ID: <abc123@example.com>',
  'From: alice@example.com',
  'To: bob@example.com',
  'Date: Mon, 01 Jan 2024 12:00:00 +0000',
  'Subject: Hello',
  '',
  'Hi Bob, how are you?',
].join('\r\n');

const MULTIPART_EMAIL = [
  'Message-ID: <multi@example.com>',
  'From: alice@example.com',
  'To: bob@example.com, carol@example.com',
  'CC: dave@example.com',
  'Date: Tue, 02 Jan 2024 10:00:00 +0000',
  'Subject: Group',
  'MIME-Version: 1.0',
  'Content-Type: multipart/alternative; boundary="boundary42"',
  '',
  '--boundary42',
  'Content-Type: text/plain; charset=UTF-8',
  '',
  'Plain text body',
  '--boundary42',
  'Content-Type: text/html',
  '',
  '<p>HTML body</p>',
  '--boundary42--',
].join('\r\n');

describe('MessageParser', () => {
  describe('parseRawMessage', () => {
    it('parses Message-ID stripping angle brackets', () => {
      const msg = parseRawMessage(SIMPLE_EMAIL, 'fallback');
      expect(msg?.id).toBe('abc123@example.com');
    });

    it('uses fallback id when Message-ID is absent', () => {
      const raw = SIMPLE_EMAIL.replace(/^Message-ID.*\r\n/m, '');
      const msg = parseRawMessage(raw, 'uid-99');
      expect(msg?.id).toBe('uid-99');
    });

    it('extracts from address', () => {
      const msg = parseRawMessage(SIMPLE_EMAIL, 'x');
      expect(msg?.from).toBe('alice@example.com');
    });

    it('extracts To recipients', () => {
      const msg = parseRawMessage(SIMPLE_EMAIL, 'x');
      expect(msg?.to).toContain('bob@example.com');
    });

    it('extracts CC recipients into to array', () => {
      const msg = parseRawMessage(MULTIPART_EMAIL, 'x');
      expect(msg?.to).toContain('dave@example.com');
    });

    it('parses the date to ISO string', () => {
      const msg = parseRawMessage(SIMPLE_EMAIL, 'x');
      expect(msg?.date).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    });

    it('extracts body from simple email', () => {
      const msg = parseRawMessage(SIMPLE_EMAIL, 'x');
      expect(msg?.body).toContain('Hi Bob');
    });

    it('extracts text/plain part from multipart email', () => {
      const msg = parseRawMessage(MULTIPART_EMAIL, 'x');
      expect(msg?.body).toContain('Plain text body');
      expect(msg?.body).not.toContain('<p>');
    });

    it('returns null for empty string', () => {
      expect(parseRawMessage('', 'x')).toBeNull();
    });

    it('defaults read to false', () => {
      const msg = parseRawMessage(SIMPLE_EMAIL, 'x');
      expect(msg?.read).toBe(false);
    });

    it('initialises attachments as empty array', () => {
      const msg = parseRawMessage(SIMPLE_EMAIL, 'x');
      expect(msg?.attachments).toEqual([]);
    });
  });
});
