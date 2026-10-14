/**
 * Unit tests for the first-run checklist's Gemini key route: the switch between
 * ChatGPT and the key form, what is sent to the main process, and that a refusal
 * shows its reason.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { ChecklistViewModel } from '../../../../../../src/renderer/features/connection/view-models/checklist-view-model.ts';

/** A checklist over a fake bridge; `answer` is what connectGemini replies. */
function setup(answer: Record<string, unknown> = { ok: true }) {
  const sent: unknown[] = [];
  const status = { signedIn: false, hasModel: false, chatgpt: { connected: false, email: '' } };
  const bridge = {
    connectGemini: async (options: unknown) => (sent.push(options), answer),
    onboardingStatus: async () => status,
    onWsStatus: () => () => {},
    onMirrorStatus: () => () => {},
  };
  return { vm: new ChecklistViewModel(bridge as any), sent, status };
}

it('switches step 1 between its ways in, clearing an old error', () => {
  const { vm } = setup();
  vm.pick('gemini');
  assert.equal(vm.state.way, 'gemini');
  vm.pick('chatgpt');
  assert.deepEqual([vm.state.way, vm.state.error], ['chatgpt', '']);
});

it('sends the typed email, key and import choice, then re-reads the steps', async () => {
  const { vm, sent, status } = setup();
  vm.type({ email: 'me@x.test', key: 'AIza1' });
  Object.assign(status, { signedIn: true, hasModel: true });
  await vm.connectGemini();
  assert.deepEqual(sent, [{ email: 'me@x.test', key: 'AIza1', importLogins: true }]);
  assert.deepEqual([vm.state.connecting, vm.state.error, vm.state.progress?.hasModel], [false, '', true]);
});

it('shows why connecting on the key failed', async () => {
  const { vm } = setup({ error: 'Paste your Gemini API key.' });
  await vm.connectGemini();
  assert.deepEqual([vm.state.connecting, vm.state.error], [false, 'Paste your Gemini API key.']);
});
