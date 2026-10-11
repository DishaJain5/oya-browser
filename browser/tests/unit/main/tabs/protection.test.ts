/** Native session protection precedes all surfaces and never attaches debugging instrumentation. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Protection, exitProxy } from '../../../../src/main/tabs/protection.ts';
import { mainCtx, FakeBrowserView } from '../../support/main-ctx.cjs';
import { nativeProtectionFixture } from '../../support/native-protection.ts';
/** Compose a native engine session with ordinary application services. */
function fixture(fail = '') {
  const engine = nativeProtectionFixture(fail);
  const ctx = mainCtx({ protection: Protection });
  ctx.world = { ensure: async () => 1 };
  const view = new FakeBrowserView();
  view.webContents.session = engine.session;
  return { ...engine, ctx, view };
}
test('refuses an unconfigured session instead of declaring a tab protected', async () => {
  const { ctx, view } = fixture();
  assert.equal(await ctx.protection.setupTabCDP(view), false);
  assert.deepEqual(view.webContents.debugger.methods(), []);
});
test('native configuration protects tabs once without touching any debugger', async () => {
  const { ctx, view, session, calls } = fixture();
  ctx.protection.configureSession(session);
  ctx.protection.configureSession(session);
  assert.equal(await ctx.protection.setupTabCDP(view), true);
  assert.equal(calls.filter((name) => name === 'scripts').length, 1);
  assert.deepEqual(view.webContents.debugger.methods(), []);
  ctx.protection.resetTabCDP(view);
  assert.equal(await ctx.protection.setupTabCDP(view), true);
});
test('a stock engine gets the user agent and the tab-preload source, never a native setter', async () => {
  const { ctx, session, calls, view } = fixture();
  delete session._setOyaPreScriptPolicy;
  const agents: string[][] = [];
  session.setUserAgent = (agent: string, languages: string) => agents.push([agent, languages]);
  ctx.protection.configureSession(session);
  assert.deepEqual(calls, []);
  assert.equal(agents.length, 1);
  assert.equal(ctx.electron.app.userAgentFallback, agents[0][0], 'cross-origin iframes get the same agent');
  assert.match(ctx.protection.stockSource(session), /\S/);
  assert.equal(await ctx.protection.setupTabCDP(view), true);
  assert.deepEqual(view.webContents.debugger.methods(), []);
});
test('a native engine leaves nothing for the tab preload to run', () => {
  const { ctx, session } = fixture();
  ctx.protection.configureSession(session);
  assert.equal(ctx.protection.stockSource(session), '');
});
test('failed source installation permanently quarantines the exact session', async () => {
  const { ctx, session, view } = fixture('scripts');
  assert.throws(() => ctx.protection.configureSession(session), /engine failure/);
  assert.throws(() => ctx.protection.configureSession(session), /restart Oya/);
  assert.equal(await ctx.protection.setupTabCDP(view), false);
});
test('a popup with an unowned partition is closed', () => {
  const { ctx, view } = fixture();
  let closed = false;
  ctx.protection.protectPopup({
    webContents: view.webContents,
    destroy() {
      closed = true;
    },
  });
  assert.equal(closed, true);
});
test('analyzer is deferred until requested and runs through the isolated world', async () => {
  const { ctx, view } = fixture();
  let calls = 0;
  ctx.world.ensure = async () => calls++;
  await ctx.protection.injectScripts(view, true);
  assert.equal(calls, 0);
  await ctx.protection.injectScripts(view);
  assert.equal(calls, 1);
});
test('governed egress overrides the persona proxy', () => {
  assert.equal(
    exitProxy({ proxy: { host: 'persona.test', port: 8080 } }, { host: 'managed.test', port: 8080 }).host,
    'managed.test',
  );
});
