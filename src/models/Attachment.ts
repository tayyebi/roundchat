/** A file attachment carried inside a message. */
export interface Attachment {
  /** MIME type, e.g. "image/png" or "application/pdf". */
  mimeType: string;
  /** Human-readable file name. */
  filename: string;
  /** Size in bytes (-1 when unknown). */
  size: number;
  /** WebDAV URL where the file can be downloaded. */
  url: string;
}
