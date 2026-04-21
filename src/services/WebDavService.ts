/**
 * WebDavService.
 *
 * Uploads files to and lists files on a WebDAV server, returning public
 * download URLs that are embedded in outgoing messages as plain links.
 * Each method performs exactly one WebDAV operation.
 */
import * as FileSystem from 'expo-file-system/legacy';
import type { WebDavConfig } from '../config/env';

export interface RemoteFile {
  name: string;
  size: number;
  mimeType: string;
  url: string;
  lastModified: string;
}

export class WebDavService {
  private config: WebDavConfig;
  private credentials: { email: string; password: string } | null = null;

  constructor(config: WebDavConfig) {
    this.config = config;
  }

  setCredentials(email: string, password: string): void {
    this.credentials = { email, password };
  }

  private buildUrl(email: string, path = ''): string {
    const base = this.config.url.replace('{email}', encodeURIComponent(email));
    return base.endsWith('/') ? `${base}${path}` : `${base}/${path}`;
  }

  private authHeader(): string {
    if (!this.credentials) throw new Error('No credentials set');
    const token = btoa(`${this.credentials.email}:${this.credentials.password}`);
    return `Basic ${token}`;
  }

  /** List all files in the user's root WebDAV directory. */
  async listFiles(): Promise<RemoteFile[]> {
    if (!this.credentials) throw new Error('No credentials set');
    const url = this.buildUrl(this.credentials.email);
    const body = `<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:">
  <D:prop>
    <D:displayname/>
    <D:getcontentlength/>
    <D:getcontenttype/>
    <D:getlastmodified/>
  </D:prop>
</D:propfind>`;

    const response = await fetch(url, {
      method: 'PROPFIND',
      headers: {
        Authorization: this.authHeader(),
        Depth: '1',
        'Content-Type': 'application/xml; charset=utf-8',
      },
      body,
    });

    if (!response.ok) throw new Error(`WebDAV PROPFIND failed: ${response.status}`);
    const xml = await response.text();
    return this.parsePropfind(xml, url);
  }

  private parsePropfind(xml: string, baseUrl: string): RemoteFile[] {
    const files: RemoteFile[] = [];
    const responseRe = /<D:response>([\s\S]*?)<\/D:response>/gi;
    let m: RegExpExecArray | null;

    while ((m = responseRe.exec(xml)) !== null) {
      const block = m[1];
      const href = this.xmlValue(block, 'D:href') ?? '';
      const name = this.xmlValue(block, 'D:displayname') ?? href.split('/').pop() ?? '';
      const size = parseInt(this.xmlValue(block, 'D:getcontentlength') ?? '0', 10);
      const mimeType = this.xmlValue(block, 'D:getcontenttype') ?? 'application/octet-stream';
      const lastModified = this.xmlValue(block, 'D:getlastmodified') ?? '';

      if (!name || href.endsWith('/')) continue;

      const fileUrl = href.startsWith('http') ? href : `${baseUrl}${href}`;
      files.push({ name, size, mimeType, url: fileUrl, lastModified });
    }
    return files;
  }

  private xmlValue(xml: string, tag: string): string | null {
    const re = new RegExp(`<${tag}[^>]*>([^<]*)<\/${tag}>`, 'i');
    const m = xml.match(re);
    return m ? m[1].trim() : null;
  }

  /**
   * Upload a local file to WebDAV.
   * Returns the public URL of the uploaded file.
   */
  async uploadFile(localUri: string, filename: string, mimeType: string): Promise<string> {
    if (!this.credentials) throw new Error('No credentials set');
    const remoteUrl = this.buildUrl(this.credentials.email, encodeURIComponent(filename));

    const result = await FileSystem.uploadAsync(remoteUrl, localUri, {
      httpMethod: 'PUT',
      headers: {
        Authorization: this.authHeader(),
        'Content-Type': mimeType,
      },
      uploadType: FileSystem.FileSystemUploadType.BINARY_CONTENT,
    });

    if (result.status < 200 || result.status >= 300) {
      throw new Error(`Upload failed with status ${result.status}`);
    }

    return remoteUrl;
  }

  /** Delete a file by its remote URL. */
  async deleteFile(url: string): Promise<void> {
    const response = await fetch(url, {
      method: 'DELETE',
      headers: { Authorization: this.authHeader() },
    });
    if (!response.ok && response.status !== 404) {
      throw new Error(`DELETE failed: ${response.status}`);
    }
  }
}
