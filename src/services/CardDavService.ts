/**
 * CardDavService.
 *
 * Fetches the user's address book from a CardDAV server and maps vCards to
 * the app's Contact model.  Each method handles exactly one HTTP operation.
 */
import type { CardDavConfig } from '../config/env';
import type { Contact } from '../models/Contact';

export class CardDavService {
  private config: CardDavConfig;
  private credentials: { email: string; password: string } | null = null;

  constructor(config: CardDavConfig) {
    this.config = config;
  }

  setCredentials(email: string, password: string): void {
    this.credentials = { email, password };
  }

  private buildUrl(email: string): string {
    return this.config.url.replace('{email}', encodeURIComponent(email));
  }

  private authHeader(): string {
    if (!this.credentials) throw new Error('No credentials set');
    const token = btoa(`${this.credentials.email}:${this.credentials.password}`);
    return `Basic ${token}`;
  }

  /** Perform a CardDAV PROPFIND to list all vCard resources. */
  async listVCardUrls(): Promise<string[]> {
    if (!this.credentials) throw new Error('No credentials set');
    const url = this.buildUrl(this.credentials.email);
    const body = `<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:carddav">
  <D:prop>
    <D:getetag/>
    <D:getcontenttype/>
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

    if (!response.ok) throw new Error(`CardDAV PROPFIND failed: ${response.status}`);
    const xml = await response.text();
    return this.parseHrefList(xml);
  }

  private parseHrefList(xml: string): string[] {
    const re = /<D:href>([^<]+\.vcf)<\/D:href>/gi;
    const hrefs: string[] = [];
    let m: RegExpExecArray | null;
    while ((m = re.exec(xml)) !== null) {
      hrefs.push(m[1]);
    }
    return hrefs;
  }

  /** Download a single vCard and parse it into a Contact. */
  async fetchContact(href: string): Promise<Contact | null> {
    const base = href.startsWith('http') ? href : `https://${this.config.url.split('/')[2]}${href}`;
    const response = await fetch(base, {
      headers: { Authorization: this.authHeader() },
    });
    if (!response.ok) return null;
    const vcard = await response.text();
    return this.parseVCard(vcard);
  }

  private parseVCard(vcard: string): Contact | null {
    const uid = this.vcardField(vcard, 'UID') ?? '';
    const fn = this.vcardField(vcard, 'FN') ?? '';
    const email = this.vcardEmailField(vcard) ?? '';
    const tel = this.vcardField(vcard, 'TEL') ?? undefined;
    const photo = this.vcardField(vcard, 'PHOTO') ?? undefined;

    if (!email) return null;
    return { id: uid || email, displayName: fn || email, email, phone: tel, avatarUrl: photo };
  }

  private vcardField(vcard: string, field: string): string | null {
    const re = new RegExp(`^${field}(?:;[^:]*)?:(.+)$`, 'im');
    const m = vcard.match(re);
    return m ? m[1].trim() : null;
  }

  private vcardEmailField(vcard: string): string | null {
    const re = /^EMAIL(?:;[^:]*)?:(.+)$/im;
    const m = vcard.match(re);
    return m ? m[1].trim().toLowerCase() : null;
  }

  /** Fetch all contacts from the address book. */
  async fetchAllContacts(): Promise<Contact[]> {
    const hrefs = await this.listVCardUrls();
    const contacts: Contact[] = [];
    for (const href of hrefs) {
      const contact = await this.fetchContact(href);
      if (contact) contacts.push(contact);
    }
    return contacts.sort((a, b) => a.displayName.localeCompare(b.displayName));
  }
}
