/** Checks the exact wire payload emitted by keel-license, including first-run reports. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseKeelPing } from '../../../../src/modules/telemetry/keel-ping.ts';

const PING = {
  install_id: '6ed27761-8487-470f-b5ae-81d1886af2b4',
  version: '0.4.0',
  os: 'macos',
  day: '2026-10-11',
  steps: 200,
  license_id: null,
};

describe('Keel ping validation', () => {
  it('accepts each supported platform and drops extra fields', () => {
    for (const os of ['macos', 'windows', 'linux']) {
      const ping = { ...PING, os };
      assert.deepEqual(parseKeelPing({ ...ping, path: '/private/project', email: 'private' }), ping);
    }
  });

  it('accepts the empty-day zero-step report a fresh install sends', () => {
    const first = { ...PING, day: '', steps: 0 };
    assert.deepEqual(parseKeelPing(first), first);
    assert.throws(() => parseKeelPing({ ...first, steps: 1 }), { field: 'day' });
  });

  it('accepts leap days, prereleases, license ids and the full Rust u32 range', () => {
    const ping = { ...PING, day: '2024-02-29', steps: 4294967295, version: '0.4.1-rc.1', license_id: 'lic-1' };
    assert.deepEqual(parseKeelPing(ping), ping);
  });

  it('rejects invalid values with the field named, without coercion', () => {
    const invalid = {
      install_id: ['nope', null, [PING.install_id]],
      version: [null, 4, {}, 'undefined', '1.0.0-' + 'x'.repeat(40)],
      os: ['MacIntel', '', null, ['macos']],
      day: ['2026-02-29', '2026-04-31', '2026-13-01', '2026-1-01', null, 20261011],
      steps: [-1, 1.5, '200', null, 4294967296],
      license_id: ['a b', [], {}, 42, 'x'.repeat(65)],
    };
    for (const [field, values] of Object.entries(invalid))
      for (const value of values) assert.throws(() => parseKeelPing({ ...PING, [field]: value }), { field });
  });

  it('requires every wire field', () => {
    for (const field of Object.keys(PING)) {
      const body = { ...PING };
      delete body[field];
      assert.throws(() => parseKeelPing(body), { field });
    }
    for (const body of [null, undefined, [], 'text', 42])
      assert.throws(() => parseKeelPing(body), { field: 'install_id' });
  });

  it('normalizes UUID case so the same install cannot be counted twice', () => {
    assert.equal(parseKeelPing({ ...PING, install_id: PING.install_id.toUpperCase() }).install_id, PING.install_id);
  });
});
