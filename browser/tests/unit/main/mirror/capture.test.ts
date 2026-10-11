/** Browser imports cannot launch a competitor or attach an internal debugging transport. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { requireOfflineSource } from '../../../../src/main/mirror/capture.ts';

it('Chromium profile import refuses Windows, where cookies are bound to the browser itself', () => {
  assert.throws(
    () => requireOfflineSource({ kind: 'chromium', name: 'Chrome' } as any, 'win32'),
    /native cookie decryption is unsupported/,
  );
});
it('Chromium profile import reads macOS profiles natively, with the Keychain key', () => {
  assert.doesNotThrow(() => requireOfflineSource({ kind: 'chromium', name: 'Chrome' } as any, 'darwin'));
});
it('offline Firefox import remains available without launching a browser', () => {
  assert.doesNotThrow(() => requireOfflineSource({ kind: 'firefox', name: 'Firefox' } as any));
});
