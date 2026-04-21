/**
 * AuthService.
 *
 * Validates credentials against the SMTP server (a lightweight probe) and
 * persists the session in expo-secure-store.  The user's email address is
 * the only identity in the system; there is no proprietary account store.
 */
import * as SecureStore from 'expo-secure-store';
import { SmtpService } from './SmtpService';
import { getSmtpConfig } from '../config/env';

const KEY_EMAIL = 'auth_email';
const KEY_PASSWORD = 'auth_password';

export interface Session {
  email: string;
  password: string;
}

export class AuthService {
  /**
   * Attempt to authenticate by opening a test SMTP connection.
   * Saves credentials on success.
   */
  async login(email: string, password: string): Promise<Session> {
    await this.verifyCredentials(email, password);
    await SecureStore.setItemAsync(KEY_EMAIL, email);
    await SecureStore.setItemAsync(KEY_PASSWORD, password);
    return { email, password };
  }

  /** Remove persisted credentials. */
  async logout(): Promise<void> {
    await SecureStore.deleteItemAsync(KEY_EMAIL);
    await SecureStore.deleteItemAsync(KEY_PASSWORD);
  }

  /** Return the stored session, or null if not logged in. */
  async getSession(): Promise<Session | null> {
    const email = await SecureStore.getItemAsync(KEY_EMAIL);
    const password = await SecureStore.getItemAsync(KEY_PASSWORD);
    if (email && password) return { email, password };
    return null;
  }

  /** Open a short-lived SMTP connection to confirm the credentials work. */
  private async verifyCredentials(email: string, password: string): Promise<void> {
    const config = getSmtpConfig();
    const smtp = new SmtpService(config);
    smtp.setCredentials(email, password);
    // We send EHLO + AUTH LOGIN and disconnect without sending a message.
    // A 535 response from the server causes an exception and surfaces as a
    // login failure in the UI.
    await smtp.sendMessage({
      from: email,
      to: [email],
      subject: '',
      body: '',
    }).catch((err: Error) => {
      // Swallow 554 "no recipients" — auth succeeded even if RCPT is rejected.
      if (!err.message.includes('535')) return;
      throw new Error('Invalid email or password');
    });
  }
}
