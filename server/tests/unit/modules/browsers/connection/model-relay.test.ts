/**
 * Unit tests for the desktop model relay: a model body goes to the run's own
 * desktop (else another desktop on the key) outside command admission, its answer
 * comes back as the raw stream, a desktop that signed out of ChatGPT means a 401,
 * a dropped connection is worth a retry, and no desktop at all is a clear refusal.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { desktopRelay } from '../../../../../src/modules/browsers/connection/model-relay.ts';
import { settleResult, failPending } from '../../../../../src/modules/browsers/connection/commands.ts';
import { registry } from '../../../../../src/modules/browsers/registry.ts';
import { LlmError } from '../../../../../src/platform/llm/transport.ts';
import { retryable } from '../../../../../src/platform/llm/transport.ts';
import { FakeSocket } from '../../../support/fakes.ts';

const KEY = 'relay-key';
const BODY = { model: 'gpt-6.1-sol', input: [], store: false, stream: true };

/** Registers a connected Oya browser of `provider` on `apiKey`; answers its socket. */
function desktop(id: string, provider = 'oya-desktop', apiKey = KEY) {
  const ws = new FakeSocket();
  registry.add(id, { ws, apiKey, name: id, clientType: 'oya', provider, persona: { id: 'p' } });
  return ws;
}

/** Answers the one outstanding llm_request on `id` the way a desktop does. */
function answer(id: string, ws: FakeSocket, result: object) {
  const sent = ws.ofType('llm_request').at(-1);
  settleResult(id, { ...result, id: sent.id }, true);
  return sent;
}

afterEach(() => {
  for (const id of ['d-1', 'd-2', 'cloud-1']) {
    failPending(id, 'disconnected', 'test over'); // no 180 s timer outlives its test
    registry.remove(id);
  }
});

describe('desktopRelay', () => {
  it('carries the body to the run’s own desktop and answers its raw stream', async () => {
    const ws = desktop('d-1');
    const pending = desktopRelay(KEY, 'd-1')(BODY);
    const sent = answer('d-1', ws, { type: 'cmd_result', ok: true, data: { status: 200, text: 'data: {}\n' } });
    assert.deepEqual(sent.params, { body: BODY });
    assert.equal(sent.action, 'llm_request');
    assert.deepEqual(await pending, { status: 200, text: 'data: {}\n' });
  });

  it('falls back to another desktop on the key, never to a cloud browser or another key', async () => {
    desktop('cloud-1', 'oya-cloud');
    desktop('d-2', 'oya-desktop', 'other-key');
    await assert.rejects(desktopRelay(KEY, 'cloud-1')(BODY), { code: 'desktop_offline', status: 422 });
    const ws = desktop('d-1');
    const pending = desktopRelay(KEY, 'cloud-1')(BODY);
    answer('d-1', ws, { type: 'cmd_result', ok: true, data: { status: 200, text: '' } });
    assert.equal((await pending).status, 200);
  });

  it('a desktop that signed out of ChatGPT is a 401, never retried', async () => {
    const ws = desktop('d-1');
    const pending = desktopRelay(KEY, 'd-1')(BODY);
    answer('d-1', ws, { type: 'cmd_result', ok: false, error: 'signed out', code: 'chatgpt_signed_out' });
    await assert.rejects(pending, (err: LlmError) => err.status === 401 && !retryable(err));
  });

  it('a desktop that dropped mid-call is worth a retry', async () => {
    desktop('d-1');
    const pending = desktopRelay(KEY, 'd-1')(BODY);
    failPending('d-1', 'disconnected', 'Browser disconnected');
    await assert.rejects(pending, (err: LlmError) => err.status === null && retryable(err));
  });

  it('a closed socket does not count as a desktop', async () => {
    const ws = desktop('d-1');
    ws.close();
    await assert.rejects(desktopRelay(KEY, 'd-1')(BODY), { code: 'desktop_offline' });
  });
});
