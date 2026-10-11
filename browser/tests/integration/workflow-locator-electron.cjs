/** Real Electron: a replay fallback locator must land on the element with the recorded name, never whoever holds its old slot. */
const assert = require('node:assert/strict');
const { app, BrowserWindow } = require('electron');
const { locatorScript } = require('../../src/main/workflow/native-locator-script.ts');
/** Suggestions in the order replay sees them: the recorded slot now holds someone else. */
const PAGE = `<!doctype html><label for=g>Guests</label><input id=g placeholder="Add guests">
<div role=listbox><div role=option id=":1a">Annabel Reyes</div><div role=option id=":1b">Anna Kowalski</div></div>
<button aria-label="Close"><span>x</span></button>`;
/** What the native target resolver reads for one candidate. */
const read = (win, candidate, el) =>
  win.webContents.executeJavaScript(locatorScript({ candidate, key: 'k', token: 't', el, hidden: false }));
app.whenReady().then(async () => {
  const win = new BrowserWindow({ show: false });
  try {
    await win.loadURL('data:text/html,' + encodeURIComponent(PAGE));
    const anna = { tag: 'div', text: 'Anna Kowalski' };
    const slot = { kind: 'css', value: '[role=listbox] > div:nth-of-type(1)' };
    assert.equal((await read(win, slot, anna)).recorded, false, 'a different person in the recorded slot is refused');
    const own = { kind: 'css', value: '[role=listbox] > div:nth-of-type(2)' };
    assert.equal((await read(win, own, anna)).recorded, true, 'the recorded person is accepted wherever they now sit');
    const id = { kind: 'css', value: '[id=":1a"]' };
    assert.equal((await read(win, id, anna)).recorded, false, 'a generated id reused by another row is refused');
    const field = { kind: 'css', value: '#g' };
    assert.equal(
      (await read(win, field, { tag: 'input', text: 'Guests' })).recorded,
      true,
      'fields keep matching by label',
    );
    const icon = { kind: 'css', value: 'button' };
    assert.equal(
      (await read(win, icon, { tag: 'button', text: 'Close' })).recorded,
      true,
      'an icon button matches by its label',
    );
    const cut = { tag: 'div', text: 'Anna Kowal...' };
    assert.equal((await read(win, own, cut)).recorded, true, 'a recorded name the recorder cut short still matches');
    console.log('PASS: replay fallbacks refuse a different name, accept the recorded one, fields and icon buttons');
    app.exit(0);
  } catch (error) {
    console.error(error);
    app.exit(1);
  }
});
