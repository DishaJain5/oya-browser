/**
 * Unit tests for answering the server's model calls: only OpenAI's Responses
 * address is ever called, always unstored and streamed, with this app's token;
 * a signed-out app and an unreachable OpenAI answer with codes the server reads;
 * an answer for a replaced socket is dropped.
 */
import { it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { answerModelRequest } from '../../../../src/main/chatgpt/relay.ts';

/** The services the relay reads, with a socket that records what it sends. */
function deps(token: string | null = 'tok') {
  const sent: any[] = [];
  const socket = { ws: {}, send: (m: unknown) => sent.push(m) };
  const chatgpt = { accessToken: async () => (token === null ? Promise.reject(Error('no')) : token) };
  return { deps: { socket, chatgpt } as any, sent };
}

afterEach(() => mock.restoreAll());

it('calls only the Responses address, unstored and streamed, whatever the body names', async () => {
  const fetch = mock.method(globalThis, 'fetch', async () => new Response('data: {}\n', { status: 200 }));
  const { deps: d, sent } = deps();
  await answerModelRequest(d, {
    id: 'r1',
    params: { body: { model: 'm', url: 'https://evil.test', store: true, stream: false } },
  });
  const [url, init] = fetch.mock.calls[0].arguments as [string, RequestInit];
  assert.equal(url, 'https://api.openai.com/v1/responses');
  assert.equal((init.headers as any).Authorization, 'Bearer tok');
  assert.deepEqual([JSON.parse(String(init.body)).store, JSON.parse(String(init.body)).stream], [false, true]);
  assert.deepEqual(sent[0], {
    type: 'cmd_result',
    id: 'r1',
    ok: true,
    data: { status: 200, text: 'data: {}\n', retryAfter: undefined },
  });
});

it('answers signed out without a call when there is no ChatGPT sign-in', async () => {
  const fetch = mock.method(globalThis, 'fetch', async () => new Response(''));
  const { deps: d, sent } = deps(null);
  await answerModelRequest(d, { id: 'r2', params: { body: {} } });
  assert.equal(fetch.mock.callCount(), 0);
  assert.equal(sent[0].code, 'chatgpt_signed_out');
});

it('answers unreachable when OpenAI cannot be reached, and drops an answer for a replaced socket', async () => {
  mock.method(globalThis, 'fetch', async () => Promise.reject(Error('down')));
  const { deps: d, sent } = deps();
  await answerModelRequest(d, { id: 'r3', params: { body: {} } });
  assert.equal(sent[0].code, 'unreachable');
  mock.method(globalThis, 'fetch', async () => {
    d.socket.ws = {};
    return new Response('x');
  });
  await answerModelRequest(d, { id: 'r4', params: { body: {} } });
  assert.equal(sent.length, 1);
});

/** A response whose stream sends `chunks` and then stays open, never closing. */
function openStream(chunks: string[]) {
  const body = new ReadableStream({
    start(controller) {
      for (const c of chunks) controller.enqueue(new TextEncoder().encode(c));
    },
  });
  return new Response(body, { status: 200 });
}

it('stops reading once the final event is whole, even split in pieces with the connection left open', async () => {
  const head = 'event: x\ndata: {"type":"response.completed","response":{"output":[{"type":"mess';
  const tail = 'age"}]}}\n\n';
  mock.method(globalThis, 'fetch', async () => openStream(['data: {"type":"response.created"}\n\n', head, tail]));
  const { deps: d, sent } = deps();
  await answerModelRequest(d, { id: 'r5', params: { body: {} } });
  assert.equal(sent[0].ok, true);
  assert.ok(sent[0].data.text.endsWith(tail), 'the whole final event, not its first piece');
});

it('gives up on a stream that goes quiet, so the server retries instead of waiting minutes', async () => {
  mock.timers.enable({ apis: ['setTimeout'] });
  mock.method(globalThis, 'fetch', async () => openStream(['data: {"type":"response.created"}\n\n']));
  const { deps: d, sent } = deps();
  const answered = answerModelRequest(d, { id: 'r6', params: { body: {} } });
  for (let i = 0; i < 5 && !sent.length; i++) {
    await new Promise((r) => setImmediate(r));
    mock.timers.tick(46_000);
  }
  await answered;
  mock.timers.reset();
  assert.equal(sent[0].code, 'unreachable');
});
