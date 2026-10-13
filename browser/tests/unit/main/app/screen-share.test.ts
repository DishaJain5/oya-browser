/**
 * Unit tests for src/main/app/screen-share.ts: sharing goes through the same
 * gate as the camera, is remembered per profile, hands over an explicitly chosen source
 * as video only, and never uses the system picker (which would skip the gate).
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { ScreenShare } from '../../../../src/main/app/screen-share.ts';
import { fixture as mediaFixture } from '../../support/media-fixture.ts';

/** A screen share over the camera fixture: its gate, profile and store, plus a capturer and the macOS switch. */
function fixture(platform = 'darwin') {
  const f = mediaFixture(platform);
  const frame = { id: 'main' };
  (f.contents as any).mainFrame = frame;
  const electron = f.deps.electron as any;
  electron.webContents = { fromFrame: (asked) => (asked === frame ? f.contents : null) };
  const picks = [];
  const picker = { pick: async (...args) => (picks.push(args), { video: { id: 'screen:0', name: 'Screen 1' } }) };
  electron.systemPreferences.getMediaAccessStatus = () => 'granted';
  const share = new ScreenShare({ ...f.deps, mediaPermissions: f.service, screenPicker: picker } as any, platform);
  const ses: any = {};
  let handler;
  ses.setDisplayMediaRequestHandler = (fn, opts) => ((handler = fn), (ses.opts = opts));
  share.install(ses);
  const request = (overrides = {}) =>
    new Promise((resolve) => handler({ frame, securityOrigin: 'https://call.test', ...overrides }, resolve));
  return { ...f, frame, electron, ses, request, share, picker, picks };
}

it('requires top-frame screen consent on the native media preflight', async () => {
  const f = fixture();
  const details = { isMainFrame: true, securityOrigin: 'https://call.test', mediaTypes: [] };
  const ask = (overrides = {}) => f.share.request(f.contents as any, { ...details, ...overrides } as any);
  assert.equal(await ask({ isMainFrame: false }), false);
  assert.equal(await ask({ securityOrigin: 'https://other.test' }), false);
  assert.deepEqual(f.prompts, []);
  f.state.response = 0;
  assert.equal(await ask(), false);
  assert.equal(f.deps.sitePermissions.has('persist:one', 'https://call.test', 'screen'), false);
  f.state.response = 1;
  assert.equal(await ask(), true);
  assert.equal(f.deps.sitePermissions.has('persist:one', 'https://call.test', 'screen'), true);
  f.state.interactive = false;
  assert.equal(await ask(), false);
});

it('installs without the system picker, so every request reaches the gate', () => {
  assert.equal(fixture().ses.opts, undefined);
});
it('remembers site consent but asks for a source on every share, as video only', async () => {
  const f = fixture();
  assert.deepEqual(await f.request({ audioRequested: true }), { video: { id: 'screen:0', name: 'Screen 1' } });
  assert.match(f.prompts[0].message, /Share your screen with https:\/\/call.test/);
  assert.equal(f.prompts[0].defaultId, 0);
  assert.deepEqual(await f.request(), { video: { id: 'screen:0', name: 'Screen 1' } });
  assert.equal(f.prompts.length, 1, 'remembered');
  assert.equal(f.picks.length, 2, 'the source is never remembered');
});
it('refuses agent, governed, background, http and iframe requesters before reading the store', async () => {
  for (const block of [
    (f) => (f.state.interactive = false),
    (f) => (f.deps.governance.configuration = {}),
    (f) => (f.deps.tabs.getActiveView = () => null),
    (f) => (f.state.url = 'http://call.test/'),
    (f) => (f.frame.id = 'child'),
  ]) {
    const f = fixture();
    f.deps.sitePermissions.grant('persist:one', 'https://call.test', ['screen']);
    block(f);
    const overrides = f.frame.id === 'child' ? { frame: { id: 'child' } } : {};
    assert.equal(await f.request(overrides), null);
    assert.deepEqual(f.prompts, []);
  }
});
it('Block answers null and remembers nothing', async () => {
  const f = fixture();
  f.state.response = 0;
  assert.equal(await f.request(), null);
  assert.equal(f.deps.sitePermissions.has('persist:one', 'https://call.test', 'screen'), false);
});
it('macOS Screen Recording denied explains once and answers null', async () => {
  const f = fixture();
  f.electron.systemPreferences.getMediaAccessStatus = () => 'denied';
  assert.equal(await f.request(), null);
  assert.equal(await f.request(), null);
  assert.equal(f.prompts.filter((p) => /System Settings/.test(p.detail)).length, 1);
});
it('a page that navigated during the dialog gets null', async () => {
  const f = fixture();
  const pending = Promise.withResolvers();
  f.deps.electron.dialog.showMessageBox = () => pending.promise;
  const answer = f.request();
  f.contents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false });
  pending.resolve({ response: 1 });
  assert.equal(await answer, null);
});
it('a picker failure answers null rather than throwing into Electron', async () => {
  const f = fixture();
  f.picker.pick = async () => {
    throw new Error('no screens');
  };
  assert.equal(await f.request(), null);
});

it('cancelling the picker never falls back to an automatic screen', async () => {
  const f = fixture();
  f.picker.pick = async () => null;
  assert.equal(await f.request(), null);
});

it('refuses a screen source if control, document or profile changes during capture selection', async () => {
  for (const revoke of [
    (f) => (f.state.interactive = false),
    (f) => f.contents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false }),
    (f) => (f.deps.persona.partitionName = () => 'persist:two'),
  ]) {
    const f = fixture();
    f.picker.pick = async () => {
      revoke(f);
      return { video: { id: 'screen:0' } };
    };
    assert.equal(await f.request(), null);
  }
});
