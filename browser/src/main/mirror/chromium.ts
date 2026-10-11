/**
 * Logins from a Chromium browser on macOS (Chrome, Arc, Brave, Edge), read from its
 * own cookie database without starting it or attaching a debugger. Chromium seals
 * cookie values with a key it keeps in the macOS Keychain ("<Browser> Safe Storage");
 * the Keychain asks the person before handing it over. Only the first-party jar is
 * read: a copy kept for another top-level site would replace the real cookie.
 */
import path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createDecipheriv, pbkdf2Sync } from 'node:crypto';
import { sqliteRows } from './sqlite.ts';
import type { ChromiumSource } from './locate.ts';
import type { ProfileCapture } from './capture.ts';
import type { SlimCookie } from './read.ts';
import * as C from './constants.ts';

/** One row of Chromium's cookies table, its sealed value as hex. */
interface ChromiumRow {
  /** The cookie's domain; a leading dot makes it a domain cookie. */
  host_key: string;
  /** Its name. */
  name: string;
  /** Its value when stored in the clear. */
  value: string;
  /** Its sealed value, hex. */
  sealed: string;
  /** Its path. */
  path: string;
  /** Expiry in microseconds since 1601. */
  expires_utc: number;
  /** 1 when it has an expiry. */
  has_expires: number;
  /** 1 when HTTPS only. */
  is_secure: number;
  /** 1 when hidden from page scripts. */
  is_httponly: number;
  /** -1 unspecified, 0 None, 1 Lax, 2 Strict. */
  samesite: number;
}

/** A row of the meta table. */
interface MetaRow {
  /** The value stored under the key. */
  value: string;
}

/** The Keychain item each browser keeps its cookie key under. */
const SAFE_STORAGE: Record<string, string> = {
  Chrome: 'Chrome Safe Storage',
  Arc: 'Arc Safe Storage',
  Brave: 'Brave Safe Storage',
  Edge: 'Microsoft Edge Safe Storage',
};

/** Chromium's sameSite numbers in the pool's spelling. */
const SAME_SITE: Record<number, string> = { 0: 'None', 1: 'Lax', 2: 'Strict' };

/** First-party cookies, their sealed value as hex. */
const COOKIE_SQL =
  "SELECT host_key, name, value, hex(encrypted_value) AS sealed, path, expires_utc, has_expires, is_secure, is_httponly, samesite FROM cookies WHERE top_frame_site_key = ''";
/** The cookie database's schema version. */
const VERSION_SQL = "SELECT value FROM meta WHERE key = 'version'";

/** The cookie key for `browser`, after the person allows Oya to read it from the Keychain. */
export async function cookieKey(browser: string): Promise<Buffer> {
  if (!Object.hasOwn(SAFE_STORAGE, browser)) throw new Error(`Import from ${browser} is not supported.`);
  const args = ['find-generic-password', '-w', '-s', SAFE_STORAGE[browser]];
  const read = promisify(execFile)('security', args, { timeout: C.KEYCHAIN_PROMPT_TIMEOUT_MS }).catch(() => null);
  const secret = (await read)?.stdout.trim();
  if (!secret)
    throw new Error(`Allow Oya to use "${SAFE_STORAGE[browser]}" in the Keychain prompt to import from ${browser}.`);
  return pbkdf2Sync(secret, C.CHROMIUM_KEY_SALT, C.CHROMIUM_KEY_ROUNDS, C.CHROMIUM_KEY_BYTES, 'sha1');
}

/** A sealed value opened with `key`; the host hash newer databases prefix is dropped. Empty when it is not v10. */
export function openValue(sealedHex: string, key: Buffer, hashed: boolean): string {
  const sealed = Buffer.from(sealedHex, 'hex');
  if (sealed.subarray(0, C.CHROMIUM_V10.length).toString() !== C.CHROMIUM_V10) return '';
  const decipher = createDecipheriv('aes-128-cbc', key, Buffer.alloc(C.CHROMIUM_KEY_BYTES, ' '));
  const plain = Buffer.concat([decipher.update(sealed.subarray(C.CHROMIUM_V10.length)), decipher.final()]);
  return (hashed ? plain.subarray(C.SHA256_BYTES) : plain).toString('utf8');
}

/** A Chromium time (microseconds since 1601) in whole epoch seconds. */
const webkitSeconds = (us: number): number =>
  Math.floor((us / C.US_PER_MS - C.WEBKIT_EPOCH_OFFSET_MS) / C.MS_PER_SECOND);

/** One row as the pool stores it, its value opened by `open`. */
export function slimChromium(row: ChromiumRow, open: (hex: string) => string): SlimCookie {
  const value = row.value || (row.sealed ? open(row.sealed) : '');
  return { name: row.name, value, domain: row.host_key, path: row.path || '/', ...chromiumFlags(row) };
}

/** A row's flags and expiry: None only on a secure cookie, since Chromium refuses an insecure None. */
function chromiumFlags(row: ChromiumRow): Omit<SlimCookie, 'name' | 'value' | 'domain' | 'path'> {
  const sameSite = SAME_SITE[row.samesite] || 'unspecified';
  return {
    secure: !!row.is_secure,
    httpOnly: !!row.is_httponly,
    sameSite: sameSite === 'None' && !row.is_secure ? 'unspecified' : sameSite,
    hostOnly: !row.host_key.startsWith('.'),
    ...(row.has_expires ? { expirationDate: webkitSeconds(row.expires_utc) } : {}),
  };
}

/** A profile's first-party cookies, opened with `key`. */
export function profileCookies(profileDir: string, key: Buffer): SlimCookie[] {
  const file = path.join(profileDir, 'Cookies');
  const version = Number(sqliteRows<MetaRow>(file, VERSION_SQL)[0]?.value ?? 0);
  const hashed = version >= C.CHROMIUM_HASHED_VALUE_VERSION;
  return sqliteRows<ChromiumRow>(file, COOKIE_SQL).map((row) =>
    slimChromium(row, (hex) => openValue(hex, key, hashed)),
  );
}

/** Every profile of `source` with its cookies, after one Keychain prompt. */
export async function captureChromium(source: ChromiumSource): Promise<ProfileCapture[]> {
  const key = await cookieKey(source.name);
  return source.profiles.map(({ dir, name, lastUsed }) => {
    const cookies = profileCookies(path.join(source.userDataDir, dir), key);
    return { profile: dir, name, lastUsed, cookies };
  });
}
