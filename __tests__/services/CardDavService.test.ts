/**
 * Tests for CardDavService.
 * Network calls are mocked with jest.spyOn on globalThis.fetch.
 */
jest.mock('expo-constants', () => ({
  expoConfig: { extra: { CARDDAV_URL: 'https://dav.example.com/users/{email}/addressbook/' } },
}));

import { CardDavService } from '../../src/services/CardDavService';

const PROPFIND_RESPONSE = `<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/users/alice%40example.com/addressbook/contact1.vcf</D:href>
    <D:propstat><D:prop><D:getcontenttype>text/vcard</D:getcontenttype></D:prop></D:propstat>
  </D:response>
</D:multistatus>`;

const VCARD = `BEGIN:VCARD
VERSION:3.0
UID:uid-001
FN:Bob Smith
EMAIL;TYPE=INTERNET:bob@example.com
TEL:+1-555-0100
END:VCARD`;

describe('CardDavService', () => {
  let service: CardDavService;

  beforeEach(() => {
    service = new CardDavService({ url: 'https://dav.example.com/users/{email}/addressbook/' });
    service.setCredentials('alice@example.com', 'secret');
  });

  it('listVCardUrls parses href list from PROPFIND response', async () => {
    jest.spyOn(globalThis, 'fetch').mockResolvedValueOnce({
      ok: true,
      status: 207,
      text: async () => PROPFIND_RESPONSE,
    } as Response);

    const hrefs = await service.listVCardUrls();
    expect(hrefs).toHaveLength(1);
    expect(hrefs[0]).toContain('.vcf');
  });

  it('fetchContact returns a Contact with parsed vcard fields', async () => {
    jest.spyOn(globalThis, 'fetch').mockResolvedValueOnce({
      ok: true,
      status: 200,
      text: async () => VCARD,
    } as Response);

    const contact = await service.fetchContact('https://dav.example.com/users/alice%40example.com/addressbook/contact1.vcf');
    expect(contact?.displayName).toBe('Bob Smith');
    expect(contact?.email).toBe('bob@example.com');
    expect(contact?.phone).toBe('+1-555-0100');
    expect(contact?.id).toBe('uid-001');
  });

  it('fetchContact returns null when server responds with error', async () => {
    jest.spyOn(globalThis, 'fetch').mockResolvedValueOnce({
      ok: false,
      status: 404,
      text: async () => 'Not Found',
    } as Response);

    const contact = await service.fetchContact('https://dav.example.com/missing.vcf');
    expect(contact).toBeNull();
  });

  it('listVCardUrls throws when PROPFIND fails', async () => {
    jest.spyOn(globalThis, 'fetch').mockResolvedValueOnce({
      ok: false,
      status: 401,
      text: async () => 'Unauthorized',
    } as Response);

    await expect(service.listVCardUrls()).rejects.toThrow('CardDAV PROPFIND failed');
  });
});
