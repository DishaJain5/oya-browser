/**
 * Unit tests for password export import: CSV rows as RFC 4180 reads them, and
 * the logins taken from each exporter's header (Passwords app, Chrome, Firefox,
 * Bitwarden), with a file that is not a passwords export refused.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { csvRows, loginsFrom } from '../../../../src/main/mirror/passwords.ts';

it('reads quoted fields holding commas, doubled quotes and line breaks', () => {
  const text = '﻿a,b\r\n"x, y","say ""hi""\nthere"\n';
  assert.deepEqual(csvRows(text), [
    ['a', 'b'],
    ['x, y', 'say "hi"\nthere'],
  ]);
});

it('keeps an empty field and a last row with no line break', () => {
  assert.deepEqual(csvRows('a,,c\n1,2,'), [
    ['a', '', 'c'],
    ['1', '2', ''],
  ]);
});

it("takes logins from the Passwords app's export", () => {
  const text = 'Title,URL,Username,Password,Notes,OTPAuth\nA,https://a.com/,ada,pw1,,\n';
  assert.deepEqual(loginsFrom(text), [{ url: 'https://a.com/', username: 'ada', password: 'pw1' }]);
});

it('takes logins from Chrome, Firefox and Bitwarden exports by their own column names', () => {
  const chrome = 'name,url,username,password,note\nb,https://b.com,bo,pw2,\n';
  const firefox = '"url","username","password","httpRealm"\n"https://c.com","cy","pw3",""\n';
  const bitwarden = 'folder,name,login_uri,login_username,login_password\n,d,https://d.com,di,pw4\n';
  assert.deepEqual(
    [chrome, firefox, bitwarden].map((text) => loginsFrom(text)[0].username),
    ['bo', 'cy', 'di'],
  );
});

it('leaves out rows with no password, such as a secure note', () => {
  assert.deepEqual(loginsFrom('url,username,password\nhttps://e.com,note-only,\n'), []);
});

it('refuses a file with no website and password columns', () => {
  assert.throws(() => loginsFrom('first,last\nAda,Lovelace\n'), /no website and password columns/);
});
