/**
 * Unit tests for src/main/app/site-permissions.ts: grants are per profile and
 * site, forgetting is per profile, and a bad file is empty, not fatal.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { SitePermissionStore } from '../../../../src/main/app/site-permissions.ts';
import { memoryFile } from '../../support/main-ctx.cjs';

describe('SitePermissionStore', () => {
  it('a grant is per partition and origin, and never leaks to another partition', () => {
    const store = new SitePermissionStore(memoryFile());
    store.grant('persist:a', 'https://call.test', ['audio']);
    store.grant('persist:a', 'https://call.test', ['video']);
    assert.equal(store.has('persist:a', 'https://call.test', 'audio'), true);
    assert.equal(store.has('persist:a', 'https://call.test', 'video'), true);
    assert.equal(store.has('persist:a', 'https://call.test', 'screen'), false);
    assert.equal(store.has('persist:a', 'https://other.test', 'audio'), false);
    assert.equal(store.has('persist:b', 'https://call.test', 'audio'), false);
  });

  it('forget clears only the given partition', () => {
    const store = new SitePermissionStore(memoryFile());
    store.grant('persist:a', 'https://call.test', ['audio']);
    store.grant('persist:b', 'https://call.test', ['audio']);
    store.forget('persist:a');
    assert.deepEqual(store.origins('persist:a'), []);
    assert.deepEqual(store.origins('persist:b'), ['https://call.test']);
  });

  it('a missing or corrupt file reads as no grants', () => {
    for (const value of [null, 'garbage', 42]) {
      const store = new SitePermissionStore(memoryFile(value as any));
      assert.equal(store.has('persist:a', 'https://call.test', 'audio'), false);
      assert.deepEqual(store.origins('persist:a'), []);
    }
  });
});
