/** Public native commands dispatch real input and editing without debugger or DOM fallbacks. */
import { it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { PageDriver } from '../../../../src/main/actions/driver.ts';
import { Keyboard } from '../../../../src/main/input/keyboard.ts';
import { Mouse } from '../../../../src/main/input/mouse.ts';
import { pageView, pageCtx, instantTimers } from '../../support/page.cjs';
afterEach(() => mock.restoreAll());
/** A real driver over native fakes; any debugger access is fatal. */
function harness() {
  instantTimers();
  const view = pageView(),
    ctx = pageCtx(view);
  Object.defineProperty(view.webContents, 'debugger', {
    get() {
      throw Error('CDP forbidden');
    },
  });
  const driver = new PageDriver({ ...ctx, keyboard: new Keyboard(process.platform), mouse: new Mouse() });
  return { view, run: (action, params) => driver.runPageAction('test', action, params, view) };
}
it('Shift+Tab reaches native input with modifiers', async () => {
  const { view, run } = harness();
  await run('key_chord', { key: 'Shift+Tab' });
  assert.deepEqual(
    view.webContents.calls.filter((c) => c[0] === 'input').map((c) => c[1]),
    [
      { type: 'keyDown', keyCode: 'Tab', modifiers: ['shift'] },
      { type: 'keyUp', keyCode: 'Tab', modifiers: ['shift'] },
    ],
  );
});
it('native select-all and editing use only explicit page methods', async () => {
  const { view, run } = harness();
  const edits = [];
  view.webContents.undo = () => edits.push('undo');
  view.webContents.redo = () => edits.push('redo');
  await run('key_chord', { key: 'ControlOrMeta+a' });
  await run('edit_text', { operation: 'undo' });
  await run('edit_text', { operation: 'redo' });
  assert.ok(view.webContents.calls.some((c) => c[0] === 'selectAll'));
  assert.deepEqual(edits, ['undo', 'redo']);
  await assert.rejects(run('edit_text', { operation: 'constructor' }), /Unsupported/);
});
it('scrolling a panel sends both signed wheel axes at the specified point', async () => {
  const { view, run } = harness();
  await run('scroll_at', { x: 20, y: 30, delta_x: 80, delta_y: -40 });
  assert.deepEqual(view.webContents.calls.find((c) => c[0] === 'input')[1], {
    type: 'mouseWheel',
    x: 20,
    y: 30,
    deltaX: -80,
    deltaY: 40,
  });
});
it('invalid chords and incomplete wheels fail before input', async () => {
  const { view, run } = harness();
  for (const key of ['F12', 'Super+a', 'Control+', 'Control+日本', 'Control+🙂'])
    await assert.rejects(run('key_chord', { key }), /Unsupported/);
  await assert.rejects(run('scroll_at', { x: 0, y: 0, delta_x: Infinity, delta_y: 0 }), /finite/);
  await assert.rejects(run('scroll_at', {}), /finite/);
  assert.deepEqual(view.webContents.calls, []);
});
