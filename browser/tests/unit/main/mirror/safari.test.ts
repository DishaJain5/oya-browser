/**
 * Unit tests for Safari as an import source: its binary cookie file written here
 * as Safari lays it out, and a macOS privacy refusal turned into the Full Disk
 * Access step that fixes it.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { parseBinaryCookies, privately, FULL_DISK_ACCESS } from '../../../../src/main/mirror/safari.ts';

/** Seconds since 2001 of 2027-01-01. */
const EXPIRY = Date.UTC(2027, 0, 1) / 1000 - 978_307_200;

/** One cookie record as Safari writes it: offsets to C strings after a 56-byte head. */
function record(flags: number, domain: string, name: string, path: string, value: string): Buffer {
  const strings = [domain, name, path, value].map((s) => Buffer.from(`${s}\0`));
  const head = Buffer.alloc(56);
  let at = head.length;
  head.writeUInt32LE(flags, 8);
  strings.forEach((s, i) => (head.writeUInt32LE(at, 16 + 4 * i), (at += s.length)));
  head.writeDoubleLE(EXPIRY, 40);
  head.writeUInt32LE(at, 0);
  return Buffer.concat([head, ...strings]);
}

/** A page of records: header, count, offsets, footer, then the records. */
function page(records: Buffer[]): Buffer {
  const head = Buffer.alloc(8 + 4 * records.length + 4);
  head.writeUInt32BE(0x100, 0);
  head.writeUInt32LE(records.length, 4);
  let at = head.length;
  records.forEach((r, i) => (head.writeUInt32LE(at, 8 + 4 * i), (at += r.length)));
  return Buffer.concat([head, ...records]);
}

/** A whole file of `pages`. */
function file(pages: Buffer[]): Buffer {
  const head = Buffer.alloc(8 + 4 * pages.length);
  head.write('cook', 0, 'latin1');
  head.writeUInt32BE(pages.length, 4);
  pages.forEach((p, i) => head.writeUInt32BE(p.length, 8 + 4 * i));
  return Buffer.concat([head, ...pages, Buffer.alloc(8)]);
}

it("reads every cookie on every page of Safari's binary cookie file", () => {
  const bytes = file([
    page([record(5, '.linkedin.com', 'li_at', '/', 'session'), record(0, 'a.test', 'pref', '', 'x')]),
    page([record(1, '.b.test', 'sid', '/app', 'y')]),
  ]);
  const cookies = parseBinaryCookies(bytes);
  assert.deepEqual(cookies[0], {
    name: 'li_at',
    value: 'session',
    domain: '.linkedin.com',
    path: '/',
    secure: true,
    httpOnly: true,
    sameSite: 'unspecified',
    hostOnly: false,
    expirationDate: Date.UTC(2027, 0, 1) / 1000,
  });
  assert.deepEqual(
    cookies.slice(1).map((c) => [c.domain, c.name, c.path, c.secure, c.httpOnly, c.hostOnly]),
    [
      ['a.test', 'pref', '/', false, false, true],
      ['.b.test', 'sid', '/app', true, false, false],
    ],
  );
});

it('refuses a file that is not a Safari cookie file', () => {
  assert.throws(() => parseBinaryCookies(Buffer.from('nope0000')), /not readable/);
});

it('turns a macOS privacy refusal into the Full Disk Access step, and leaves other errors alone', () => {
  const refused = Object.assign(new Error('EPERM: operation not permitted, open'), { code: 'EPERM' });
  assert.throws(
    () =>
      privately(() => {
        throw refused;
      }),
    { message: FULL_DISK_ACCESS },
  );
  assert.throws(
    () =>
      privately(() => {
        throw new Error('disk on fire');
      }),
    /disk on fire/,
  );
});
