/**
 * Unit tests for src/main/app/json-file.ts: a missing or corrupt file reads as
 * null, and a write is owner-only. State lives in a scratch directory.
 */
import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { JsonFile } from '../../../../src/main/app/json-file.ts';
import { PRIVATE_FILE_MODE } from '../../../../src/main/app/constants.ts';

/** The value under test. */
interface State {
  /** A number, to see it come back. */
  a: number;
}

describe('JsonFile', () => {
  let dir: string;
  before(() => (dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-json-'))));
  after(() => fs.rmSync(dir, { recursive: true, force: true }));

  /** Names a file in the scratch directory, lazily, as the app names its files. */
  const at =
    (...parts: string[]) =>
    () =>
      path.join(dir, ...parts);

  it('a missing or corrupt file reads as null', () => {
    assert.equal(new JsonFile(at('missing.json')).read(), null);
    fs.writeFileSync(path.join(dir, 'bad.json'), '{not json');
    assert.equal(new JsonFile(at('bad.json')).read(), null);
  });

  it('writes owner-only and reads back what it wrote', () => {
    const file = new JsonFile<State>(at('state.json'));
    file.write({ a: 1 });
    assert.deepEqual(file.read(), { a: 1 });
    if (process.platform !== 'win32') assert.equal(fs.statSync(at('state.json')()).mode & 0o777, PRIVATE_FILE_MODE);
  });

  it('a failed write keeps the running state, never throws', () => {
    const file = new JsonFile<State>(at('no-such-dir', 'state.json'));
    assert.doesNotThrow(() => file.write({ a: 1 }));
    assert.equal(file.read(), null);
  });
});
