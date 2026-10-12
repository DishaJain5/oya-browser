/** The passthrough keeps the front door's rules: persona, files, web-only egress, and id translation per tab. */
import { test, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { CdpPassthrough, passesThrough } from '../../../../src/main/app/cdp-passthrough.ts';

/** A tab whose debugger answers with the CDP id 'CDP' and echoes params back. */
function fixture(allowed = true) {
  const calls: unknown[][] = [];
  const dbg = Object.assign(new EventEmitter(), {
    attached: false,
    isAttached() {
      return this.attached;
    },
    attach() {
      this.attached = true;
    },
    async sendCommand(method: string, params?: object, child?: string) {
      calls.push([method, params, child]);
      return method === 'Target.getTargetInfo' ? { targetInfo: { targetId: 'CDP' } } : { echo: params };
    },
  });
  const webContents = { isDestroyed: () => false, debugger: dbg } as any;
  const through = new CdpPassthrough({ view: () => ({ webContents }), allowedURL: () => allowed });
  return { through, dbg, calls };
}
afterEach(() => {
  delete process.env.OYA_CDP_PASSTHROUGH;
});
test('page domains pass through; browser-wide ones and other tabs do not', () => {
  assert.equal(passesThrough('Runtime.callFunctionOn'), true);
  assert.equal(passesThrough('Target.setAutoAttach'), true);
  for (const method of ['Browser.close', 'Target.createTarget', 'Storage.getCookies', 'Debugger.enable', 'IO.read'])
    assert.equal(passesThrough(method), false, method);
});
test('OYA_CDP_PASSTHROUGH=0 turns the passthrough off', () => {
  process.env.OYA_CDP_PASSTHROUGH = '0';
  assert.equal(passesThrough('Page.enable'), false);
});
test('the persona owns identity: overrides are refused before reaching the tab', async () => {
  const f = fixture();
  await assert.rejects(f.through.send('t', 'Emulation.setUserAgentOverride', { userAgent: 'x' }), /persona/);
  await assert.rejects(f.through.send('t', 'Emulation.setTimezoneOverride', { timezoneId: 'UTC' }), /persona/);
  assert.equal(f.calls.length, 0);
});
test('loads that skip navigation are refused', async () => {
  const f = fixture();
  await assert.rejects(f.through.send('t', 'Network.loadNetworkResource', { url: 'https://x.test' }), /egress/);
  assert.equal(f.calls.length, 0);
});
test('navigation opens only web addresses the egress rules allow', async () => {
  await assert.rejects(fixture().through.send('t', 'Page.navigate', { url: 'file:///etc/passwd' }), /http/);
  await assert.rejects(fixture(false).through.send('t', 'Page.navigate', { url: 'https://x.test' }), /egress/);
  await assert.rejects(
    fixture(false).through.send('t', 'Fetch.continueRequest', { requestId: '1', url: 'https://x.test' }),
    /egress/,
  );
  await fixture().through.send('t', 'Page.navigate', { url: 'https://x.test' });
});
test("the tab's main frame id is the front door's target id, both ways", async () => {
  const f = fixture();
  const reply = await f.through.send('t', 'Page.navigate', { url: 'https://x.test', frameId: 't' });
  assert.deepEqual(f.calls.at(-1), ['Page.navigate', { url: 'https://x.test', frameId: 'CDP' }, undefined]);
  assert.deepEqual(reply, { echo: { url: 'https://x.test', frameId: 't' } });
});
test('events carry the translated id until the watch stops', async () => {
  const f = fixture();
  await f.through.send('t', 'Page.enable', {});
  const seen: unknown[] = [];
  const stop = f.through.watch('t', (method, params, child) => seen.push([method, params, child]));
  f.dbg.emit('message', {}, 'Page.frameNavigated', { frame: { id: 'CDP' } }, 'child');
  stop();
  f.dbg.emit('message', {}, 'Page.frameNavigated', { frame: { id: 'CDP' } });
  assert.deepEqual(seen, [['Page.frameNavigated', { frame: { id: 't' } }, 'child']]);
});
