/**
 * Tests for the environment configuration module.
 * We mock expo-constants to inject controlled values.
 */
jest.mock('expo-constants', () => ({
  expoConfig: {
    extra: {
      IMAP_HOST: 'imap.test.com',
      IMAP_PORT: '993',
      IMAP_TLS: 'true',
      POP3_HOST: 'pop3.test.com',
      POP3_PORT: '995',
      POP3_TLS: 'false',
      POP3_POLL_INTERVAL: '60',
      SMTP_HOST: 'smtp.test.com',
      SMTP_PORT: '587',
      SMTP_TLS: 'true',
      CARDDAV_URL: 'https://carddav.test.com/users/{email}/addressbook/',
      WEBDAV_URL: 'https://webdav.test.com/files/{email}/',
    },
  },
}));

import {
  getImapConfig,
  getPop3Config,
  getSmtpConfig,
  getCardDavConfig,
  getWebDavConfig,
} from '../../src/config/env';

describe('env config', () => {
  describe('getImapConfig', () => {
    it('returns IMAP config when IMAP_HOST is set', () => {
      const c = getImapConfig();
      expect(c).not.toBeNull();
      expect(c?.host).toBe('imap.test.com');
      expect(c?.port).toBe(993);
      expect(c?.tls).toBe(true);
    });
  });

  describe('getPop3Config', () => {
    it('returns POP3 config with parsed values', () => {
      const c = getPop3Config();
      expect(c.host).toBe('pop3.test.com');
      expect(c.port).toBe(995);
      expect(c.tls).toBe(false);
      expect(c.pollInterval).toBe(60);
    });
  });

  describe('getSmtpConfig', () => {
    it('returns SMTP config with port as number', () => {
      const c = getSmtpConfig();
      expect(c.host).toBe('smtp.test.com');
      expect(c.port).toBe(587);
    });
  });

  describe('getCardDavConfig', () => {
    it('returns the CardDAV URL', () => {
      const c = getCardDavConfig();
      expect(c.url).toContain('carddav.test.com');
    });
  });

  describe('getWebDavConfig', () => {
    it('returns the WebDAV URL', () => {
      const c = getWebDavConfig();
      expect(c.url).toContain('webdav.test.com');
    });
  });
});
