/** Desktop provisioning fails before publishing unpinned or unverified engine paths to release jobs. */
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createHash } from 'node:crypto';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { installDesktopEngines } from './setup-native-desktop.mjs';

/** Only installer boundaries are replaced; tests never download or execute a browser. */
function fixture(t, platform = 'darwin') {
  const root = mkdtempSync(join(tmpdir(), 'oya-desktop-setup-test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const bytes = 'pinned fixture archive',
    sha = createHash('sha256').update(bytes).digest('hex');
  const env = { RUNNER_TEMP: root, GITHUB_ENV: join(root, 'env') },
    calls = [];
  for (const name of ['MACOS_ARM64', 'MACOS_X64', 'WINDOWS_X64']) {
    env[`OYA_NATIVE_${name}_URL`] = `https://example.invalid/${name}.tar.gz`;
    env[`OYA_NATIVE_${name}_SHA256`] = sha;
  }
  writeFileSync(env.GITHUB_ENV, '');
  const deps = {
    env,
    platform,
    download: (url, path) => {
      calls.push(['download', url]);
      writeFileSync(path, bytes);
    },
    extract: (archive, directory) => {
      calls.push(['extract', directory]);
    },
    verify: (os, arch, directory) => {
      calls.push(['verify', os, arch, directory]);
    },
  };
  return { deps, calls, exported: () => readFileSync(env.GITHUB_ENV, 'utf8') };
}

test('a universal Mac job validates both pins before downloading either engine', async (t) => {
  for (const value of ['', 'http://example.invalid/native.tgz', 'https://user:secret@example.invalid/native.tgz']) {
    const f = fixture(t);
    f.deps.env.OYA_NATIVE_MACOS_X64_URL = value;
    await assert.rejects(installDesktopEngines(f.deps), /OYA_NATIVE_MACOS_X64_URL/);
    assert.deepEqual(f.calls, []);
    assert.equal(f.exported(), '');
  }
});

test('missing digests and unsupported platforms fail before downloading', async (t) => {
  const f = fixture(t, 'win32');
  f.deps.env.OYA_NATIVE_WINDOWS_X64_SHA256 = '';
  await assert.rejects(installDesktopEngines(f.deps), /OYA_NATIVE_WINDOWS_X64_SHA256/);
  await assert.rejects(installDesktopEngines({ ...f.deps, platform: 'linux' }), /macOS or Windows/);
  assert.deepEqual(f.calls, []);
  assert.equal(f.exported(), '');
});

test('checksum failure prevents extraction and native execution', async (t) => {
  const f = fixture(t);
  f.deps.env.OYA_NATIVE_MACOS_ARM64_SHA256 = '0'.repeat(64);
  await assert.rejects(installDesktopEngines(f.deps), /checksum mismatch: darwin-arm64/);
  assert.deepEqual(
    f.calls.map(([name]) => name),
    ['download'],
  );
  assert.equal(f.exported(), '');
});

test('download and extraction errors cannot publish a distribution root', async (t) => {
  for (const stage of ['download', 'extract']) {
    const f = fixture(t);
    f.deps[stage] = () => {
      throw Error(`failed ${stage}`);
    };
    await assert.rejects(installDesktopEngines(f.deps), new RegExp(`failed ${stage}`));
    assert.ok(!f.calls.some(([name]) => name === 'verify'));
    assert.equal(f.exported(), '');
  }
});

test('failure of either Mac native probe leaves the job environment unchanged', async (t) => {
  for (const failedArch of ['arm64', 'x64']) {
    const f = fixture(t),
      verify = f.deps.verify;
    f.deps.verify = (platform, arch, directory) => {
      verify(platform, arch, directory);
      if (arch === failedArch) throw Error('Native capability or architecture mismatch');
    };
    await assert.rejects(installDesktopEngines(f.deps), /Native capability/);
    assert.equal(f.exported(), '');
  }
});

test('macOS publishes one root containing both verified architecture directories', async (t) => {
  const f = fixture(t),
    root = await installDesktopEngines(f.deps);
  assert.deepEqual(
    f.calls.filter(([name]) => name === 'verify'),
    [
      ['verify', 'darwin', 'arm64', join(root, 'darwin-arm64')],
      ['verify', 'darwin', 'x64', join(root, 'darwin-x64')],
    ],
  );
  assert.equal(f.exported(), `OYA_NATIVE_DISTRIBUTIONS=${root}\n`);
});

test('Windows needs only its x64 pin and publishes the directory used by beforePack', async (t) => {
  const f = fixture(t, 'win32');
  delete f.deps.env.OYA_NATIVE_MACOS_ARM64_URL;
  delete f.deps.env.OYA_NATIVE_MACOS_X64_URL;
  const root = await installDesktopEngines(f.deps);
  assert.deepEqual(
    f.calls.filter(([name]) => name === 'verify'),
    [['verify', 'win32', 'x64', join(root, 'win32-x64')]],
  );
  assert.equal(f.exported(), `OYA_NATIVE_DISTRIBUTIONS=${root}\n`);
});

test('release jobs provision desktop engines before packaging and signing', () => {
  const workflow = readFileSync('.github/workflows/deploy-prod.yaml', 'utf8');
  const windows = workflow.slice(workflow.indexOf('  build-windows:'), workflow.indexOf('  build-mac:'));
  const mac = workflow.slice(workflow.indexOf('  build-mac:'), workflow.indexOf('  build-image:'));
  for (const job of [windows, mac]) assert.match(job, /node scripts\/setup-native-desktop\.mjs/);
  assert.ok(windows.indexOf('Provision native') < windows.indexOf('Build NSIS installer'));
  assert.ok(mac.indexOf('Provision native') < mac.indexOf('Import signing certificate'));
  for (const name of ['MACOS_ARM64', 'MACOS_X64', 'WINDOWS_X64'])
    for (const pin of ['URL', 'SHA256']) assert.ok(workflow.includes(`vars.OYA_NATIVE_${name}_${pin}`));
});
