/** A log out keeps the account sealed so the welcome screen can sign back in with one click, as a mail app does. */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { shellHandlers } from '../../../../src/main/ipc/index.ts';
import { DeepLinks } from '../../../../src/main/app/deep-links.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

/** A keychain that seals reversibly, so a test can tell sealed from plain. */
function fakeKeychain(available = true) {
  return {
    isEncryptionAvailable: () => available,
    getSelectedStorageBackend: () => 'keychain',
    encryptString: (text: string) => Buffer.from(`sealed:${text}`),
    decryptString: (data: Buffer) => {
      const text = data.toString();
      if (!text.startsWith('sealed:')) throw new Error('not ours');
      return text.slice('sealed:'.length);
    },
  };
}

const ada = { email: 'ada@example.com', name: 'Ada', project: { name: 'Lab' } };

describe('saved accounts', () => {
  let ctx: any, call: any;
  beforeEach(() => {
    ctx = mainCtx();
    ctx.electron.safeStorage = fakeKeychain();
    ctx.deepLinks = new DeepLinks(ctx);
    ctx.tabs = { leaveBrowsingMode() {} };
    const handlers: any = shellHandlers(ctx);
    call = (channel, ...args) => handlers[channel]({}, ...args);
  });
  /** Signs in as `apiKey` (as Ada unless told otherwise), then logs out. */
  async function signOutOf(apiKey: string, account: object = ada, serverUrl = 'wss://oya.test/ws') {
    ctx.config.values = { ...ctx.config.values, serverUrl, apiKey, persona: 'work', account, signedOut: false };
    await call('sign-out');
  }

  it('a log out offers the account again, naming the person, email, project and server', async () => {
    await signOutOf('oya_key_1');
    assert.deepEqual(
      (await call('saved-accounts')).map(({ id: _id, ...shown }) => shown),
      [{ label: 'Ada', email: 'ada@example.com', project: 'Lab', host: 'oya.test' }],
    );
  });
  it('an account known only by email shows it once, as the label', async () => {
    await signOutOf('oya_key_1', { email: 'bo@example.com' });
    const [saved] = await call('saved-accounts');
    assert.deepEqual([saved.label, saved.email, saved.project], ['bo@example.com', '', '']);
  });

  it('keeps the key only sealed, and never hands it to the welcome screen', async () => {
    await signOutOf('oya_key_1');
    assert.doesNotMatch(JSON.stringify(ctx.config.values.savedAccounts), /"oya_key_1/);
    assert.doesNotMatch(JSON.stringify(await call('saved-accounts')), /oya_key_1/);
  });

  it('one click signs back in to the same server, key and persona', async () => {
    await signOutOf('oya_key_1');
    const [saved] = await call('saved-accounts');
    const connects = ctx.socket.connects || 0;
    assert.equal(await call('use-saved-account', saved.id), true);
    assert.equal(ctx.config.values.apiKey, 'oya_key_1');
    assert.equal(ctx.config.values.serverUrl, 'wss://oya.test/ws');
    assert.equal(ctx.config.values.persona, 'work');
    assert.equal(ctx.config.values.signedOut, false);
    assert.equal(ctx.socket.connects, connects + 1);
  });

  it('logging out of the same account again keeps one entry, newest first', async () => {
    await signOutOf('oya_key_1');
    await signOutOf('oya_key_2', { email: 'bo@example.com' });
    await signOutOf('oya_key_1');
    assert.deepEqual(
      (await call('saved-accounts')).map((a) => a.label),
      ['Ada', 'bo@example.com'],
    );
  });

  it('remembers at most ten accounts', async () => {
    for (let i = 0; i < 12; i++) await signOutOf(`oya_key_${i}`, { email: `p${i}@example.com` });
    const labels = (await call('saved-accounts')).map((a) => a.label);
    assert.equal(labels.length, 10);
    assert.equal(labels[0], 'p11@example.com');
  });

  it('without a real keychain nothing is kept, so no key lingers in plain text', async () => {
    ctx.electron.safeStorage = fakeKeychain(false);
    await signOutOf('oya_key_1');
    assert.deepEqual(await call('saved-accounts'), []);
  });

  it('remove forgets the account on this browser', async () => {
    await signOutOf('oya_key_1');
    const [saved] = await call('saved-accounts');
    assert.deepEqual(await call('forget-saved-account', saved.id), []);
    assert.deepEqual(await call('saved-accounts'), []);
  });

  it('an account this keychain cannot open is dropped with a reason, and nothing is signed in', async () => {
    await signOutOf('oya_key_1');
    const [saved] = await call('saved-accounts');
    ctx.config.values.savedAccounts[0].sealedKey = Buffer.from('from another Mac').toString('base64');
    assert.throws(() => call('use-saved-account', saved.id), /Sign in again/);
    assert.equal(ctx.config.values.apiKey, '');
    assert.deepEqual(await call('saved-accounts'), []);
  });

  it('an unknown id signs nothing in', async () => {
    assert.throws(() => call('use-saved-account', 'nope'), /Sign in again/);
    assert.equal(ctx.config.values.apiKey, undefined);
  });
});
