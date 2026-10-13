/** Native settings QA reuses the isolated Oya shell fixture; no network or real account mutation. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
/** Read the current shell renderer through Oya's native API. */
const read = (win, source) => win.webContents.executeJavaScript(source);
/** Let the native compositor paint settled layout before capturing. */
const paint = async (win) => {
  await read(
    win,
    'Promise.all(document.querySelector(".shell-dialog").getAnimations({subtree:true}).filter(a => a.effect.getTiming().iterations !== Infinity).map(a => a.finished.catch(() => {})))',
  );
  await read(win, 'new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))');
};
/** Every category stays reachable and appearance controls still update the live shared theme. */
module.exports = async function settings({ win, click, until }) {
  await click(win, '#conn-pill');
  await until(() =>
    read(win, '!document.querySelector("#profile-section").hidden && !!document.querySelector(".settings-layout")'),
  );
  assert.equal(await read(win, 'document.querySelector("#settings-general").hidden'), false);
  assert.equal(await read(win, 'document.querySelector("#settings-logins").hidden'), true);
  await paint(win);
  fs.writeFileSync('/private/tmp/oya-settings-general-dark.png', (await win.webContents.capturePage()).toPNG());
  await click(win, '#settings-nav-logins');
  await until(() => read(win, '!document.querySelector("#settings-logins").hidden'));
  assert.equal(await read(win, 'document.querySelector(".import-history-disclosure").open'), false);
  await click(win, '.import-history-disclosure summary');
  await until(() => read(win, 'document.querySelector(".import-history-disclosure").open'));
  assert.equal(await read(win, 'document.querySelectorAll("#import-history li").length'), 1);
  await click(win, '.import-history-disclosure summary');
  await paint(win);
  fs.writeFileSync('/private/tmp/oya-settings-logins-dark.png', (await win.webContents.capturePage()).toPNG());
  await click(win, '#settings-nav-account');
  await until(() => read(win, '!document.querySelector("#settings-account").hidden'));
  await click(win, '#profile-advanced summary');
  await until(() => read(win, 'document.querySelector("#profile-advanced").open'));
  assert.ok(await read(win, 'document.querySelector("#connection-edit").getBoundingClientRect().height'));
  await click(win, '#settings-nav-general');
  await until(() => read(win, '!document.querySelector("#settings-general").hidden'));
  win.focus();
  win.webContents.focus();
  await read(win, 'document.querySelector("#theme-preference").focus()');
  win.webContents.sendInputEvent({ type: 'keyDown', keyCode: 'L' });
  win.webContents.sendInputEvent({ type: 'char', keyCode: 'l' });
  win.webContents.sendInputEvent({ type: 'keyUp', keyCode: 'L' });
  await until(() => read(win, 'document.documentElement.dataset.theme === "light"'));
  await paint(win);
  fs.writeFileSync('/private/tmp/oya-settings-general-light.png', (await win.webContents.capturePage()).toPNG());
  const bounds = win.getBounds();
  win.setMinimumSize(600, 500);
  win.setSize(660, 650);
  await until(() => read(win, 'window.innerWidth <= 660'));
  await paint(win);
  assert.equal(
    await read(
      win,
      'document.querySelector(".settings-layout").scrollWidth <= document.querySelector(".settings-layout").clientWidth',
    ),
    true,
  );
  fs.writeFileSync('/private/tmp/oya-settings-small.png', (await win.webContents.capturePage()).toPNG());
  await click(win, '#shell-dialog-close');
  await until(() => read(win, 'document.querySelector("#shell-overlay").hidden'));
  win.setBounds(bounds);
  assert.equal(await read(win, 'document.querySelector("#btn-dev .button-label").textContent'), 'Assistant');
  assert.equal(await read(win, 'document.querySelector("#control-agent").textContent.trim()'), 'AI Mode');
  console.log('Oya Settings: categories, import history, live theme, small-window layout and compact labels passed');
};
