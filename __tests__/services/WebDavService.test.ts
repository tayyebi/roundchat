/**
 * Tests for WebDavService.
 * Network calls and expo-file-system are mocked.
 */
jest.mock('expo-constants', () => ({
  expoConfig: { extra: { WEBDAV_URL: 'https://webdav.example.com/files/{email}/' } },
}));

jest.mock('expo-file-system/legacy', () => ({
  uploadAsync: jest.fn(),
  FileSystemUploadType: { BINARY_CONTENT: 0 },
}));

import * as FileSystem from 'expo-file-system/legacy';
import { WebDavService } from '../../src/services/WebDavService';

const PROPFIND_XML = `<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/files/alice%40example.com/report.pdf</D:href>
    <D:propstat>
      <D:prop>
        <D:displayname>report.pdf</D:displayname>
        <D:getcontentlength>204800</D:getcontentlength>
        <D:getcontenttype>application/pdf</D:getcontenttype>
        <D:getlastmodified>Mon, 01 Jan 2024 00:00:00 GMT</D:getlastmodified>
      </D:prop>
    </D:propstat>
  </D:response>
</D:multistatus>`;

describe('WebDavService', () => {
  let service: WebDavService;

  beforeEach(() => {
    service = new WebDavService({ url: 'https://webdav.example.com/files/{email}/' });
    service.setCredentials('alice@example.com', 'secret');
    jest.clearAllMocks();
  });

  it('listFiles returns parsed RemoteFile objects', async () => {
    jest.spyOn(globalThis, 'fetch').mockResolvedValueOnce({
      ok: true,
      status: 207,
      text: async () => PROPFIND_XML,
    } as Response);

    const files = await service.listFiles();
    expect(files).toHaveLength(1);
    expect(files[0].name).toBe('report.pdf');
    expect(files[0].size).toBe(204800);
    expect(files[0].mimeType).toBe('application/pdf');
  });

  it('listFiles throws when PROPFIND returns error status', async () => {
    jest.spyOn(globalThis, 'fetch').mockResolvedValueOnce({
      ok: false,
      status: 403,
      text: async () => 'Forbidden',
    } as Response);

    await expect(service.listFiles()).rejects.toThrow('WebDAV PROPFIND failed');
  });

  it('uploadFile calls FileSystem.uploadAsync with correct params', async () => {
    (FileSystem.uploadAsync as jest.Mock).mockResolvedValueOnce({ status: 201 });

    const url = await service.uploadFile('file:///local/doc.pdf', 'doc.pdf', 'application/pdf');
    expect(FileSystem.uploadAsync).toHaveBeenCalledWith(
      expect.stringContaining('doc.pdf'),
      'file:///local/doc.pdf',
      expect.objectContaining({ httpMethod: 'PUT' }),
    );
    expect(url).toContain('doc.pdf');
  });

  it('uploadFile throws when server returns error status', async () => {
    (FileSystem.uploadAsync as jest.Mock).mockResolvedValueOnce({ status: 500 });
    await expect(service.uploadFile('file:///x', 'x.pdf', 'application/pdf')).rejects.toThrow('Upload failed');
  });

  it('deleteFile calls DELETE method', async () => {
    jest.spyOn(globalThis, 'fetch').mockResolvedValueOnce({ ok: true, status: 204 } as Response);
    await service.deleteFile('https://webdav.example.com/files/alice%40example.com/report.pdf');
    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining('report.pdf'),
      expect.objectContaining({ method: 'DELETE' }),
    );
  });
});
