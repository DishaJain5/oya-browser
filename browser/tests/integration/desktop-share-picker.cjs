/** Native Oya picker QA: real windows, preload IPC and trusted pointer input; no browser automation provider. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { BrowserWindow, session } = require('electron');
const { ScreenPicker } = require('../../src/main/screen-share/index.ts');

/** Bounded observation of native windows and rendered controls. */
async function until(read) {
  for (let count = 0; count < 100; count++) {
    const value = await read();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw Error('Native share picker did not become ready');
}

/** Locate only the browser-owned picker, never the person's normal browser windows. */
async function sheet() {
  return until(() =>
    BrowserWindow.getAllWindows().find(
      (window) => window.isVisible() && window.webContents.getURL().endsWith('/share-picker.html'),
    ),
  );
}

/** A native trusted click drives the isolated preload's ordinary controls. */
async function click(window, selector) {
  const point = await window.webContents.executeJavaScript(`(() => {
    const node = document.querySelector(${JSON.stringify(selector)});
    const rect = node.getBoundingClientRect();
    return { x: Math.round(rect.x + rect.width / 2), y: Math.round(rect.y + rect.height / 2) };
  })()`);
  window.webContents.sendInputEvent({ type: 'mouseDown', button: 'left', clickCount: 1, ...point });
  window.webContents.sendInputEvent({ type: 'mouseUp', button: 'left', clickCount: 1, ...point });
}

/** Select a category and a source, verifying Share is disabled until the explicit choice. */
async function choose(kind, screenshot = false) {
  const window = await sheet();
  const wc = window.webContents;
  assert.equal(await wc.executeJavaScript('document.getElementById("share").disabled'), true);
  await click(window, `#${kind}`);
  await click(window, '.source');
  await until(() => wc.executeJavaScript('!document.getElementById("share").disabled'));
  if (screenshot && process.env.OYA_PICKER_SCREENSHOT)
    fs.writeFileSync(process.env.OYA_PICKER_SCREENSHOT, (await wc.capturePage()).toPNG());
  await click(window, '#share');
}

/** Fake OS entries use only pixels from the disposable test page; real desktop enumeration is never called. */
async function installPicker(deps, window) {
  const thumbnail = await window.webContents.capturePage();
  const sources = [
    { id: 'window:fixture', name: 'Project notes', thumbnail },
    { id: 'screen:fixture', name: 'Test display', thumbnail },
  ];
  deps.electron.BrowserWindow = BrowserWindow;
  deps.electron.session = session;
  deps.electron.desktopCapturer.getSources = async () => sources;
  deps.appDir = require('node:path').resolve(__dirname, '../..');
  deps.tabs.list = [
    { id: 1, title: 'Project notes', url: window.webContents.getURL(), view: { webContents: window.webContents } },
  ];
  deps.screenPicker = new ScreenPicker(deps);
  return sources;
}

/** Verify each category resolves exactly its chosen native source, independently of live display capture. */
async function categories(deps, window, sources) {
  for (const [kind, expected] of [
    ['window', sources[0]],
    ['screen', sources[1]],
  ]) {
    const pending = deps.screenPicker.pick(window.webContents, 'https://call.test', () => true);
    await choose(kind);
    assert.equal((await pending).video, expected);
  }
}

/** Every share shows a fresh picker, Escape cancels, and native capture receives only the selected tab. */
async function captureChecks(request, state) {
  const capturing = request();
  await choose('tab', true);
  assert.equal(await capturing, 'captured');
  const cancelled = request();
  const window = await sheet();
  assert.equal(await window.webContents.executeJavaScript('document.getElementById("share").disabled'), true);
  window.webContents.sendInputEvent({ type: 'keyDown', keyCode: 'Escape' });
  window.webContents.sendInputEvent({ type: 'keyUp', keyCode: 'Escape' });
  assert.match(await cancelled, /AbortError|NotAllowedError/);
  const revoked = request();
  await sheet();
  state.human = false;
  assert.match(await revoked, /AbortError|NotAllowedError/);
}

module.exports = { installPicker, categories, captureChecks };
