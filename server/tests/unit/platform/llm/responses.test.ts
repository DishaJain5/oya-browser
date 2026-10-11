/**
 * Unit tests for platform/llm/responses.ts: a /responses base URL is picked, the
 * request is never stored and is streamed, only `response.completed` counts, a
 * spent plan allowance is final while other failures are retried, and a refusal
 * takes the shared error contract.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { responses } from '../../../../src/platform/llm/responses.ts';
import { providerFor } from '../../../../src/platform/llm/index.ts';
import { LlmError, retryable } from '../../../../src/platform/llm/transport.ts';

const BASE = 'https://api.openai.com/v1/responses';

/** A finished event stream of `events`. */
const stream = (...events: object[]) => events.map((e) => `event: x\ndata: ${JSON.stringify(e)}\n\n`).join('');

/** Stubs fetch to answer `text` with `status`. */
const answer = (text: string, status = 200) =>
  mock.method(globalThis, 'fetch', async () => new Response(text, { status }));

/** One step of the agent. */
const STEP = {
  baseUrl: BASE,
  apiKey: 'access-token',
  model: 'gpt-6.1-sol',
  messages: [
    { role: 'system', content: 'Be careful.' },
    { role: 'user', content: 'Open the page' },
  ],
  tools: [{ type: 'function', function: { name: 'navigate', parameters: { type: 'object' } } }],
};

/** The completed event for a response that says `text`. */
const completed = (text: string) => ({
  type: 'response.completed',
  response: {
    output: [{ type: 'message', content: [{ type: 'output_text', text }] }],
    usage: { input_tokens: 5, output_tokens: 2 },
  },
});

describe('Responses provider', () => {
  afterEach(() => mock.restoreAll());

  it('serves a /responses base URL, and only that', () => {
    assert.equal(providerFor(BASE).name, 'responses');
    assert.equal(providerFor('https://api.openai.com/v1').name, 'openai');
  });

  it('posts an unstored, streamed request with the bearer token and reads the completed answer', async () => {
    const fetch = answer(stream({ type: 'response.created' }, completed('Done')));
    const reply = await responses.send(STEP);
    const [url, init] = fetch.mock.calls[0].arguments as [string, RequestInit];
    const body = JSON.parse(String(init.body));
    assert.equal(url, BASE);
    assert.equal((init.headers as any).Authorization, 'Bearer access-token');
    assert.deepEqual(
      [body.store, body.stream, body.instructions, body.tools[0].name],
      [false, true, 'Be careful.', 'navigate'],
    );
    assert.equal(reply.choices[0].message.content, 'Done');
    assert.deepEqual(reply.usage, { prompt_tokens: 5, completion_tokens: 2 });
  });

  it('gathers the output from the per-item events when the closing event carries none', async () => {
    const item = { type: 'function_call', call_id: 'c9', name: 'navigate', arguments: '{"url":"x"}' };
    answer(
      stream(
        { type: 'response.output_item.done', item: { type: 'reasoning', summary: [] } },
        { type: 'response.output_item.done', item },
        { type: 'response.completed', response: { status: 'completed', usage: { input_tokens: 1, output_tokens: 1 } } },
      ),
    );
    const reply = await responses.send(STEP);
    assert.equal(reply.choices[0].message.tool_calls[0].function.name, 'navigate');
    assert.deepEqual(reply.usage, { prompt_tokens: 1, completion_tokens: 1 });
  });

  it('sends through the transport when one is given, never fetching itself, and refuses as a direct call would', async () => {
    const fetch = mock.method(globalThis, 'fetch', async () => new Response('never', { status: 500 }));
    const carried: unknown[] = [];
    const transport = async (body) => (carried.push(body), { status: 200, text: stream(completed('Relayed')) });
    const reply = await responses.send({ ...STEP, apiKey: 'chatgpt-desktop', transport });
    assert.equal(reply.choices[0].message.content, 'Relayed');
    assert.equal(fetch.mock.callCount(), 0);
    assert.deepEqual([(carried[0] as any).store, (carried[0] as any).stream], [false, true]);
    const refused = async () => ({ status: 429, text: '{"error":"slow down"}', retryAfter: '7' });
    await assert.rejects(responses.send({ ...STEP, transport: refused }), (err: LlmError) => {
      assert.equal(err.status, 429);
      assert.equal(err.retryAfterMs, 7000);
      assert.doesNotMatch(err.message, /slow down/);
      return true;
    });
  });

  it('treats a spent plan allowance as final, not worth a retry', async () => {
    const failed = {
      type: 'response.failed',
      response: { error: { code: 'subscription_sharing_usage_limit_exceeded' } },
    };
    answer(stream({ type: 'response.created' }, failed));
    await assert.rejects(responses.send(STEP), (err: LlmError) => err.status === 402 && !retryable(err));
  });

  it('retries a stream that failed otherwise or ended before completing', async () => {
    answer(stream({ type: 'response.failed', response: { error: { code: 'server_error' } } }));
    await assert.rejects(responses.send(STEP), (err: LlmError) => retryable(err));
    answer(stream({ type: 'response.created' }));
    await assert.rejects(responses.send(STEP), (err: LlmError) => err.status === null && retryable(err));
  });

  it('a refused request takes the shared error, without echoing what the endpoint said', async () => {
    answer('{"error":"secret detail"}', 401);
    await assert.rejects(responses.send(STEP), (err: LlmError) => err.status === 401 && !/secret/.test(err.message));
  });
});
