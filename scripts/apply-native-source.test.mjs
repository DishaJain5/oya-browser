/** Native source overlays require exact baseline and patch bytes before changing any repository. */
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { applyNativeSource } from './apply-native-source.mjs';

/** Fixture digests use the same public SHA-256 contract as the source manifest. */
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');

/** Two independent repositories expose accidental partial application on later validation failure. */
function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'oya-source-overlay-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const source = join(root, 'src'),
    overlay = join(root, 'overlay');
  mkdirSync(source);
  mkdirSync(overlay);
  const repositories = ['engine', 'runtime'].map((name) => repository(source, overlay, name));
  writeFileSync(join(overlay, 'manifest.json'), JSON.stringify({ repositories }));
  return { source, overlay, repositories };
}

/** Tiny real git repositories exercise git apply without network or user Git configuration. */
function repository(source, overlay, name) {
  const directory = join(source, name),
    baseline = 'before\n';
  mkdirSync(directory);
  execFileSync('git', ['init', '--quiet', directory]);
  writeFileSync(join(directory, 'value.txt'), baseline);
  const patch = 'diff --git a/value.txt b/value.txt\n--- a/value.txt\n+++ b/value.txt\n@@ -1 +1 @@\n-before\n+after\n';
  writeFileSync(join(overlay, `${name}.patch`), patch);
  return {
    path: name,
    patch: `${name}.patch`,
    sha256: hash(patch),
    baselineFiles: { 'value.txt': hash(baseline) },
    newFiles: [],
  };
}

test('verified patches apply to every exact baseline', (t) => {
  const f = fixture(t);
  applyNativeSource(f.source, f.overlay);
  for (const repo of f.repositories)
    assert.equal(readFileSync(join(f.source, repo.path, 'value.txt'), 'utf8'), 'after\n');
});

test('a later baseline mismatch leaves the earlier repository unchanged', (t) => {
  const f = fixture(t);
  writeFileSync(join(f.source, 'runtime/value.txt'), 'local work\n');
  assert.throws(() => applyNativeSource(f.source, f.overlay), /Source baseline mismatch: runtime/);
  assert.equal(readFileSync(join(f.source, 'engine/value.txt'), 'utf8'), 'before\n');
  assert.equal(readFileSync(join(f.source, 'runtime/value.txt'), 'utf8'), 'local work\n');
});

test('a later patch checksum mismatch prevents any patch application', (t) => {
  const f = fixture(t);
  writeFileSync(join(f.overlay, 'runtime.patch'), 'tampered patch');
  assert.throws(() => applyNativeSource(f.source, f.overlay), /Source patch digest mismatch: runtime/);
  assert.equal(readFileSync(join(f.source, 'engine/value.txt'), 'utf8'), 'before\n');
});

test('an existing newly introduced file is never overwritten', (t) => {
  const f = fixture(t);
  f.repositories[1].newFiles.push('new.cc');
  writeFileSync(join(f.overlay, 'manifest.json'), JSON.stringify({ repositories: f.repositories }));
  writeFileSync(join(f.source, 'runtime/new.cc'), 'local work');
  assert.throws(() => applyNativeSource(f.source, f.overlay), /would overwrite runtime\/new.cc/);
  assert.equal(readFileSync(join(f.source, 'engine/value.txt'), 'utf8'), 'before\n');
  assert.equal(readFileSync(join(f.source, 'runtime/new.cc'), 'utf8'), 'local work');
});
