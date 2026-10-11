/**
 * Unit tests for the no-account sign-up: the proof of work it solves, and the
 * server's per-network limit named for what it means.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { signUp, solve } from '../../../../src/main/chatgpt/signup.ts';

it('finds a nonce whose hash starts with the zeros asked for', () => {
  const nonce = solve({ challenge: 'c', difficulty: 2 });
  assert.ok(createHash('sha256').update(`c${nonce}`).digest('hex').startsWith('00'));
});

it('signs up with the solved puzzle, and names a used-up network limit', async () => {
  const bodies: any[] = [];
  const ok = (async (url: string, init?: RequestInit) => {
    if (url.endsWith('/challenge')) return new Response(JSON.stringify({ challenge: 'c', difficulty: 1 }));
    bodies.push(JSON.parse(String(init!.body)));
    return new Response(JSON.stringify({ api_key: 'k', claim_url: 'https://o.test/claim#k' }));
  }) as typeof fetch;
  assert.deepEqual(await signUp('wss://o.test/ws', 'me@x.test', ok), {
    apiKey: 'k',
    claimUrl: 'https://o.test/claim#k',
  });
  assert.equal(bodies[0].email, 'me@x.test');
  const limited = (async (url: string) =>
    url.endsWith('/challenge')
      ? new Response(JSON.stringify({ challenge: 'c', difficulty: 1 }))
      : new Response('{}', { status: 429 })) as typeof fetch;
  await assert.rejects(signUp('wss://o.test/ws', 'me@x.test', limited), /Too many sign-ups/);
});
