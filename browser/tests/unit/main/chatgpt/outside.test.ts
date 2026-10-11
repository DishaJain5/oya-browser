/**
 * Unit tests for opening the ChatGPT sign-in outside Oya: when Oya is the default
 * browser the page must never come back to Oya itself, where it cannot finish.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { openOutside } from '../../../../src/main/chatgpt/outside.ts';

/** Electron with Oya as the default browser or not; records what the default browser was asked to open. */
function electronWith(oyaIsDefault: boolean) {
  const opened: string[] = [];
  const electron = {
    app: { isDefaultProtocolClient: () => oyaIsDefault },
    shell: { openExternal: async (url: string) => void opened.push(url) },
  };
  return { deps: { electron } as any, opened };
}

it('uses the default browser when it is not Oya', async () => {
  const { deps, opened } = electronWith(false);
  await openOutside(deps, 'https://auth.test/', 'darwin', async () => assert.fail('no other browser needed'));
  assert.deepEqual(opened, ['https://auth.test/']);
});

it('on a Mac where Oya is the default, opens the first usual browser installed, never Oya', async () => {
  const { deps, opened } = electronWith(true);
  const ran: string[] = [];
  const run = async (_file: string, args: string[]) => {
    ran.push(args[1]);
    if (args[1] !== 'Brave Browser') throw Error('Unable to find application');
  };
  await openOutside(deps, 'https://auth.test/', 'darwin', run);
  assert.deepEqual(ran, ['Google Chrome', 'Arc', 'Brave Browser']);
  assert.deepEqual(opened, []);
});

it('on Windows where Oya is the default, opens Edge', async () => {
  const { deps, opened } = electronWith(true);
  await openOutside(deps, 'https://auth.test/', 'win32');
  assert.deepEqual(opened, ['microsoft-edge:https://auth.test/']);
});

it('says so when a Mac has no other browser at all', async () => {
  const { deps } = electronWith(true);
  await assert.rejects(
    openOutside(deps, 'https://auth.test/', 'darwin', async () => {
      throw Error('none');
    }),
    /No other browser/,
  );
});
