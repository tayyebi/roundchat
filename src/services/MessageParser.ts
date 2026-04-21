/**
 * MessageParser.
 *
 * Converts a raw RFC 5322 email string into the app's internal Message model.
 * Only plain-text parts are extracted; MIME multipart parsing is simplified.
 */
import type { Message } from '../models/Message';
import type { Attachment } from '../models/Attachment';

/** Extract the value of a header field (case-insensitive). */
function header(raw: string, name: string): string {
  const re = new RegExp(`^${name}:\\s*(.+)`, 'im');
  const m = raw.match(re);
  return m ? m[1].trim() : '';
}

/** Split a comma-separated address list into individual addresses. */
function splitAddresses(field: string): string[] {
  if (!field) return [];
  return field
    .split(',')
    .map((a) => {
      const m = a.match(/<([^>]+)>/);
      return (m ? m[1] : a).trim().toLowerCase();
    })
    .filter(Boolean);
}

/** Extract all attachment stubs referenced in the body via X-WebDAV-URL headers. */
function extractAttachments(raw: string): Attachment[] {
  const attachments: Attachment[] = [];
  const re = /Content-Disposition: attachment[^\r\n]*\r\nX-WebDAV-URL: ([^\r\n]+)\r\nContent-Type: ([^\r\n;]+)[^\r\n]*\r\nContent-Description: ([^\r\n]+)/gi;
  let m: RegExpExecArray | null;
  while ((m = re.exec(raw)) !== null) {
    attachments.push({
      url: m[1].trim(),
      mimeType: m[2].trim(),
      filename: m[3].trim(),
      size: -1,
    });
  }
  return attachments;
}

/** Extract the plain-text body from a (possibly multipart) email. */
function extractBody(raw: string): string {
  const headerEnd = raw.indexOf('\r\n\r\n');
  if (headerEnd === -1) return '';
  const body = raw.slice(headerEnd + 4);

  const contentType = header(raw, 'Content-Type');
  if (!contentType.includes('multipart/')) {
    return body.trim();
  }

  // Multipart: find boundary and return the first text/plain part.
  const boundaryMatch = contentType.match(/boundary="?([^";]+)"?/i);
  if (!boundaryMatch) return body.trim();
  const boundary = '--' + boundaryMatch[1];

  const parts = body.split(boundary);
  for (const part of parts) {
    if (/Content-Type:\s*text\/plain/i.test(part)) {
      const partBodyStart = part.indexOf('\r\n\r\n');
      if (partBodyStart !== -1) {
        return part.slice(partBodyStart + 4).replace(/--$/, '').trim();
      }
    }
  }
  return '';
}

/**
 * Parse a raw email string into a Message.
 * Returns null if the raw string cannot be parsed.
 */
export function parseRawMessage(raw: string, fallbackId: string): Message | null {
  if (!raw) return null;

  const messageId = header(raw, 'Message-ID').replace(/[<>]/g, '') || fallbackId;
  const from = splitAddresses(header(raw, 'From'))[0] ?? '';
  const toHeader = header(raw, 'To');
  const ccHeader = header(raw, 'CC');
  const to = [...splitAddresses(toHeader), ...splitAddresses(ccHeader)];
  const dateStr = header(raw, 'Date');
  const body = extractBody(raw);
  const attachments = extractAttachments(raw);

  return {
    id: messageId,
    from,
    to,
    date: dateStr ? new Date(dateStr).toISOString() : new Date().toISOString(),
    body,
    attachments,
    read: false,
  };
}
