/** Real Electron, trusted keys: Enter on a highlighted suggestion records a click on that suggestion, by name. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { app, BrowserWindow } = require('electron');
const analyzer = fs
  .readFileSync(path.join(__dirname, '../../scripts/analyzer.js'), 'utf8')
  .replace('__OYA_ATTR__', 'data-oya-test')
  .replace('__OYA_RECORD__', 'true');
/** A guest field whose arrow keys move the highlight (aria-activedescendant), as calendar apps do. */
const PAGE = `<!doctype html><label for=g>Guests</label>
<input id=g role=combobox aria-autocomplete=list aria-controls=list autocomplete=off>
<div id=list role=listbox></div><script>
const people=['Annabel Reyes','Anna Kowalski','Hanna Lee'];let at=-1;const g=document.getElementById('g'),list=document.getElementById('list');
g.addEventListener('input',()=>{list.innerHTML=people.map((p,i)=>'<div role=option id=opt'+i+'>'+p+'</div>').join('');at=-1;});
g.addEventListener('keydown',e=>{if(e.key==='ArrowDown'){at=Math.min(at+1,people.length-1);g.setAttribute('aria-activedescendant','opt'+at);e.preventDefault();}
 if(e.key==='Enter'&&at>=0){window.picked=people[at];e.preventDefault();}});
</script>`;
/** One trusted key press through Chromium's input pipeline. */
function key(win, keyCode) {
  for (const type of ['keyDown', 'keyUp']) win.webContents.sendInputEvent({ type, keyCode });
}
app.whenReady().then(async () => {
  const win = new BrowserWindow({ show: true, width: 600, height: 400 });
  try {
    await win.loadURL('data:text/html,' + encodeURIComponent(PAGE));
    await win.webContents.executeJavaScript(analyzer);
    await win.webContents.executeJavaScript(
      'window.steps=[];window.__acRecordSink=b=>steps.push(...b.steps);window.__acRecordStart();document.getElementById("g").focus()',
    );
    win.focus();
    win.webContents.focus();
    for (const c of 'Ann') win.webContents.sendInputEvent({ type: 'char', keyCode: c });
    await win.webContents.executeJavaScript('document.getElementById("g").dispatchEvent(new Event("input"))');
    key(win, 'Down');
    key(win, 'Down');
    key(win, 'Return');
    await new Promise((resolve) => setTimeout(resolve, 300));
    const { steps, picked } = await win.webContents.executeJavaScript('({steps, picked: window.picked})');
    assert.equal(picked, 'Anna Kowalski', 'the page picked the second suggestion');
    const last = steps.at(-1);
    assert.equal(
      last.action,
      'click',
      `Enter on a suggestion is a click step, got ${JSON.stringify(steps.map((s) => s.action))}`,
    );
    assert.match(last.el.text, /Anna Kowalski/);
    assert.ok(
      !steps.some((s) => s.action === 'press_key' && s.key === 'Enter'),
      'no bare Enter that would take the first suggestion',
    );
    console.log('PASS: Enter on a highlighted suggestion records a click on it by name');
    app.exit(0);
  } catch (error) {
    console.error(error);
    app.exit(1);
  }
});
