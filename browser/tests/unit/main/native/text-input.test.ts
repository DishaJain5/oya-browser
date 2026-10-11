/** Composed text uses the acknowledged engine capability, or stock Electron's own insertion on desktop. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { insertNativeText } from '../../../../src/main/native/text-input.ts';
test('text insertion prefers the native engine, refuses destroyed pages and awaits acknowledgement', async () => {
  const sent: string[] = [];
  let destroyed = false,
    acknowledge: () => void = () => {};
  const page = {
    webContents: {
      isDestroyed: () => destroyed,
      insertText: async () => {
        throw Error('stock insertion must not run on a native engine');
      },
      _insertTextOya: undefined as undefined | ((text: string) => Promise<void>),
    },
  };
  page.webContents._insertTextOya = async (text) => {
    sent.push(text);
    await new Promise<void>((resolve) => {
      acknowledge = resolve;
    });
  };
  let completed = false;
  const task = insertNativeText(page, '日本語 🙂').then(() => {
    completed = true;
  });
  await Promise.resolve();
  assert.equal(completed, false);
  acknowledge();
  await task;
  assert.equal(completed, true);
  assert.deepEqual(sent, ['日本語 🙂']);
  destroyed = true;
  await assert.rejects(insertNativeText(page, 'gone'), /destroyed/);
  assert.deepEqual(sent, ['日本語 🙂']);
});
test('a stock engine types through Electron insertText', async () => {
  const typed: string[] = [];
  const page = { webContents: { isDestroyed: () => false, insertText: async (text: string) => void typed.push(text) } };
  await insertNativeText(page, 'desktop');
  assert.deepEqual(typed, ['desktop']);
});
test('native cancellation is an error, never a successful empty insertion', async () => {
  const page = {
    webContents: {
      isDestroyed: () => false,
      insertText: async () => {},
      _insertTextOya: async () => {
        throw Error('Text input document was replaced');
      },
    },
  };
  await assert.rejects(insertNativeText(page, 'text'), /document was replaced/);
});
