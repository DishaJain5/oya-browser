/**
 * Safari as an import source on macOS: its logins (Cookies.binarycookies), bookmarks
 * (Bookmarks.plist) and history (History.db), read from its own files. macOS keeps
 * these private to apps the person has given Full Disk Access, so a refusal is turned
 * into the one step that fixes it.
 */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import type { ProfileCapture } from './capture.ts';
import type { SlimCookie } from './read.ts';
import * as C from './constants.ts';

/** Safari as a mirror source: one profile, read from disk. */
export interface SafariSource {
  /** The stable source id. */
  id: 'safari';
  /** The display name. */
  name: string;
  /** Tells capture to read Safari's own files. */
  kind: 'safari';
  /** Safari's data directory. */
  userDataDir: string;
  /** Its one profile. */
  profiles: Array<Omit<ProfileCapture, 'cookies' | 'chromeVersion'>>;
  /** Reads its cookies. */
  capture: () => ProfileCapture[];
}

/** What the person is told when macOS keeps Safari's files from Oya. */
export const FULL_DISK_ACCESS =
  'macOS keeps Safari data private. Open System Settings > Privacy & Security > Full Disk Access, turn on Oya Browser, restart it, then import again.';

/** Safari's data directory. */
const safariDir = (): string => path.join(os.homedir(), 'Library', 'Safari');
/** Where Safari keeps its cookies, under the home directory. */
const SAFARI_COOKIES = [
  'Library',
  'Containers',
  'com.apple.Safari',
  'Data',
  'Library',
  'Cookies',
  'Cookies.binarycookies',
];
/** Safari's cookie file. */
const cookieFile = (): string => path.join(os.homedir(), ...SAFARI_COOKIES);

/** `read()`, with a macOS privacy refusal explained as Full Disk Access. */
export function privately<T>(read: () => T): T {
  try {
    return read();
  } catch (error) {
    if (/EPERM|EACCES|not permitted/i.test(String((error as Error).message)))
      throw new Error(FULL_DISK_ACCESS, { cause: error });
    throw error;
  }
}

/** The C string at `at` in `buf`. */
function cString(buf: Buffer, at: number): string {
  const end = buf.indexOf(0, at);
  return buf.toString('utf8', at, end < 0 ? buf.length : end);
}

/** One cookie record (starting at `at` in `page`) as the pool stores it. */
function slimSafari(page: Buffer, at: number): SlimCookie {
  const text = (name: keyof typeof C.SAFARI_RECORD) =>
    cString(page, at + page.readUInt32LE(at + C.SAFARI_RECORD[name]));
  const domain = text('domain');
  const expires = Math.floor(page.readDoubleLE(at + C.SAFARI_RECORD.expiry) + C.MAC_EPOCH_OFFSET_S);
  const named = { name: text('name'), value: text('value'), domain, path: text('path') || '/' };
  return { ...named, ...safariFlags(page.readUInt32LE(at + C.SAFARI_RECORD.flags), domain), expirationDate: expires };
}

/** A record's flags in the pool's spelling; Safari's file carries no sameSite. */
function safariFlags(flags: number, domain: string): Pick<SlimCookie, 'secure' | 'httpOnly' | 'sameSite' | 'hostOnly'> {
  const secure = !!(flags & C.SAFARI_SECURE);
  return {
    secure,
    httpOnly: !!(flags & C.SAFARI_HTTP_ONLY),
    sameSite: 'unspecified',
    hostOnly: !domain.startsWith('.'),
  };
}

/** Every cookie on one page of the file. */
function pageCookies(page: Buffer): SlimCookie[] {
  const count = page.readUInt32LE(C.SAFARI_PAGE_HEADER);
  const offsets = Array.from({ length: count }, (_, i) =>
    page.readUInt32LE(C.SAFARI_PAGE_HEADER + C.UINT32_BYTES * (i + 1)),
  );
  return offsets.map((at) => slimSafari(page, at));
}

/** Every cookie in a Cookies.binarycookies file: a big-endian page table, then little-endian pages. */
export function parseBinaryCookies(buf: Buffer): SlimCookie[] {
  if (buf.toString('latin1', 0, C.UINT32_BYTES) !== 'cook') throw new Error('Safari cookie file is not readable');
  const pages = buf.readUInt32BE(C.UINT32_BYTES);
  const sizes = Array.from({ length: pages }, (_, i) => buf.readUInt32BE(C.SAFARI_TABLE_START + C.UINT32_BYTES * i));
  let at = C.SAFARI_TABLE_START + C.UINT32_BYTES * pages;
  return sizes.flatMap((size) => pageCookies(buf.subarray(at, (at += size))));
}

/** Safari's cookies, refused with the Full Disk Access step when macOS keeps them private. */
export function readSafariCookies(file = cookieFile()): SlimCookie[] {
  return privately(() => parseBinaryCookies(fs.readFileSync(file)));
}

/** A property list file as JSON, by macOS's plutil. */
export function plistJson(file: string): unknown {
  return JSON.parse(execFileSync('plutil', ['-convert', 'json', '-o', '-', file], { encoding: 'utf8' }));
}

/** Safari as a source; null off macOS or without Safari. */
export function safariSource(): SafariSource | null {
  if (process.platform !== 'darwin' || !fs.existsSync('/Applications/Safari.app')) return null;
  const profile = { profile: 'default', name: 'default', lastUsed: true };
  const capture = () => [{ ...profile, cookies: readSafariCookies() }];
  return { id: 'safari', name: 'Safari', kind: 'safari', userDataDir: safariDir(), profiles: [profile], capture };
}
