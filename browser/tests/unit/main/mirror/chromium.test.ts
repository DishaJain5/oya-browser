/**
 * Unit tests for reading Chromium's own cookie database on macOS: values sealed
 * as Chrome seals them (v10, AES-128-CBC with the Keychain-derived key, and the
 * host hash newer databases prefix), first-party rows only, mapped for the pool.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createCipheriv, createHash, pbkdf2Sync } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { openValue, profileCookies, slimChromium } from '../../../../src/main/mirror/chromium.ts';

/** A key derived as Chrome derives it from its Safe Storage secret. */
const KEY = pbkdf2Sync('keychain-secret', 'saltysalt', 1003, 16, 'sha1');
/** 2027-01-01 in Chromium's microseconds since 1601. */
const EXPIRES = (Date.UTC(2027, 0, 1) + 11_644_473_600_000) * 1000;

/** `value` sealed as Chrome seals it, with the host hash when `host` is given; hex. */
function seal(value: string, host?: string): string {
  const plain = Buffer.concat([
    host ? createHash('sha256').update(host).digest() : Buffer.alloc(0),
    Buffer.from(value),
  ]);
  const cipher = createCipheriv('aes-128-cbc', KEY, Buffer.alloc(16, ' '));
  return Buffer.concat([Buffer.from('v10'), cipher.update(plain), cipher.final()]).toString('hex');
}

it('opens a v10 value, dropping the host hash of a newer database', () => {
  assert.equal(openValue(seal('session'), KEY, false), 'session');
  assert.equal(openValue(seal('session', '.a.test'), KEY, true), 'session');
  assert.equal(openValue('763131', KEY, false), '', 'a value this reader cannot open is left empty');
});

it('maps a row for the pool: expiry in seconds, host-only, and None only when secure', () => {
  const row = { host_key: 'a.test', name: 'n', value: 'plain', sealed: '', path: '/', expires_utc: EXPIRES };
  const flags = { has_expires: 1, is_secure: 0, is_httponly: 1, samesite: 0 };
  const cookie = slimChromium({ ...row, ...flags }, () => assert.fail('a clear value is not opened'));
  assert.deepEqual(cookie, {
    name: 'n',
    value: 'plain',
    domain: 'a.test',
    path: '/',
    secure: false,
    httpOnly: true,
    sameSite: 'unspecified',
    hostOnly: true,
    expirationDate: Date.UTC(2027, 0, 1) / 1000,
  });
  assert.equal(slimChromium({ ...row, ...flags, is_secure: 1 }, () => '').sameSite, 'None');
});

it("reads a profile's first-party cookies from its database, never a copy kept inside another site", (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-chromium-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const row = (name: string, sealed: string, site = '') =>
    `('.linkedin.com', '${name}', '', X'${sealed}', '/', ${EXPIRES}, 1, 1, 1, 0, '${site}')`;
  const sql = `CREATE TABLE meta(key, value); INSERT INTO meta VALUES ('version', '24');
    CREATE TABLE cookies(host_key, name, value, encrypted_value, path, expires_utc, has_expires, is_secure, is_httponly, samesite, top_frame_site_key);
    INSERT INTO cookies VALUES ${row('li_at', seal('mine', '.linkedin.com'))}, ${row('bcookie', seal('theirs', '.linkedin.com'), 'https://prnewswire.com')};`;
  try {
    execFileSync('sqlite3', [path.join(dir, 'Cookies'), sql], { stdio: 'ignore' });
  } catch {
    return t.skip('no sqlite3 on this machine');
  }
  assert.deepEqual(
    profileCookies(dir, KEY).map((c) => [c.name, c.value, c.sameSite]),
    [['li_at', 'mine', 'None']],
  );
});
