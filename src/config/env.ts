/**
 * Environment configuration.
 *
 * All values are read from Expo's `extra` block (app.config.js/app.json)
 * which is populated from process.env at build time.  This module is the
 * single source of truth for every server setting used throughout the app.
 * Nothing outside this file should reference `process.env` or
 * `Constants.expoConfig.extra` directly.
 */
import Constants from 'expo-constants';

function extra(): Record<string, string> {
  return (Constants.expoConfig?.extra as Record<string, string>) ?? {};
}

function required(key: string): string {
  const value = extra()[key] ?? '';
  if (!value) {
    throw new Error(`Required environment variable "${key}" is not set.`);
  }
  return value;
}

function optional(key: string, fallback = ''): string {
  return extra()[key] ?? fallback;
}

function optionalBool(key: string, fallback = true): boolean {
  const v = optional(key, String(fallback));
  return v === 'true' || v === '1';
}

function optionalInt(key: string, fallback: number): number {
  const v = optional(key, String(fallback));
  const n = parseInt(v, 10);
  return isNaN(n) ? fallback : n;
}

export interface ImapConfig {
  host: string;
  port: number;
  tls: boolean;
}

export interface Pop3Config {
  host: string;
  port: number;
  tls: boolean;
  pollInterval: number;
}

export interface SmtpConfig {
  host: string;
  port: number;
  tls: boolean;
}

export interface CardDavConfig {
  url: string;
}

export interface WebDavConfig {
  url: string;
}

export function getImapConfig(): ImapConfig | null {
  const host = optional('IMAP_HOST');
  if (!host) return null;
  return {
    host,
    port: optionalInt('IMAP_PORT', 993),
    tls: optionalBool('IMAP_TLS', true),
  };
}

export function getPop3Config(): Pop3Config {
  return {
    host: required('POP3_HOST'),
    port: optionalInt('POP3_PORT', 995),
    tls: optionalBool('POP3_TLS', true),
    pollInterval: optionalInt('POP3_POLL_INTERVAL', 30),
  };
}

export function getSmtpConfig(): SmtpConfig {
  return {
    host: required('SMTP_HOST'),
    port: optionalInt('SMTP_PORT', 465),
    tls: optionalBool('SMTP_TLS', true),
  };
}

export function getCardDavConfig(): CardDavConfig {
  return { url: required('CARDDAV_URL') };
}

export function getWebDavConfig(): WebDavConfig {
  return { url: required('WEBDAV_URL') };
}
