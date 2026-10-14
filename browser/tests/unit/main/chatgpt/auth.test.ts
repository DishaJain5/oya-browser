/**
 * Unit tests for the app's ChatGPT sign-in: the code is exchanged under the
 * issued client (never the registration one) and sealed; a cancelled sign-in
 * stores nothing; callers at the same time share one refresh; the host id and
 * issued client are kept and reused, even after a refused exchange.
 */
import { it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ChatgptAuth } from '../../../../src/main/chatgpt/auth.ts';

const PLAN = 'openid email offline_access chatgpt.tokens.use.direct';

/** An unsigned ID token carrying `claims`. */
const idToken = (claims: object) =>
  ['{"alg":"none"}', JSON.stringify(claims), 's'].map((p) => Buffer.from(p).toString('base64url')).join('.');

/** A sign-in over fakes; the browser comes back with `query(state, nonce)`. */
function setup(query: (state: string, nonce: string) => Record<string, string>, tokens: object = {}) {
  let saved: any = null;
  const store = { load: () => saved, save: (v: unknown) => (saved = v), clear: () => (saved = null) };
  let opened = '';
  const values: Record<string, unknown> = {};
  const config = { values, merge: (c: object) => Object.assign(values, c), save() {} };
  const electron = {
    app: { getPath: () => '/tmp', isDefaultProtocolClient: () => false },
    safeStorage: {},
    shell: { openExternal: async (u: string) => void (opened = u) },
  };
  const fetch = mock.fn(
    async () =>
      new Response(JSON.stringify({ access_token: 'a', refresh_token: 'r', expires_in: 3600, scope: PLAN, ...tokens })),
  );
  const listen = async () => ({
    redirectUri: 'http://127.0.0.1:5/auth/callback',
    close() {},
    get done() {
      const url = new URL(opened);
      return Promise.resolve(query(url.searchParams.get('state')!, url.searchParams.get('nonce')!));
    },
  });
  const auth = new ChatgptAuth({ config, electron } as any, {
    fetch: fetch as never,
    listen: listen as never,
    store: store as never,
  });
  return { auth, fetch, values, saved: () => saved, opened: () => opened };
}

it('exchanges the code under the issued client and keeps the credential sealed', async () => {
  let nonce = '';
  const { auth, fetch, saved } = setup((state, n) => ((nonce = n), { state, code: 'c', client_id: 'oaiapp_x' }), {});
  fetch.mock.mockImplementation(
    async () =>
      new Response(
        JSON.stringify({
          access_token: 'a',
          refresh_token: 'r',
          expires_in: 3600,
          scope: PLAN,
          id_token: idToken({ nonce, email: 'me@x.test' }),
        }),
      ),
  );
  assert.deepEqual(await auth.signIn(), { connected: true, email: 'me@x.test' });
  const form = new URLSearchParams(String((fetch.mock.calls[0].arguments as any[])[1].body));
  assert.equal(form.get('client_id'), 'oaiapp_x');
  assert.equal(saved().client_id, 'oaiapp_x');
});

it('refuses a first sign-in that came back without an issued client, and a cancelled one, storing nothing', async () => {
  const bare = setup((state) => ({ state, code: 'c' }));
  await assert.rejects(bare.auth.signIn(), /did not finish/);
  assert.equal(bare.fetch.mock.callCount(), 0);
  const cancelled = setup((state) => ({ state, error: 'access_denied' }));
  await assert.rejects(cancelled.auth.signIn(), /cancelled/);
  assert.equal(cancelled.saved(), null);
});

it('callers at the same time share one refresh of an expired token', async () => {
  const { auth, fetch } = setup(() => ({}));
  (auth as any).store.save({ client_id: 'oaiapp_x', refresh_token: 'r', access_token: 'old', expires_at: 0 });
  const [a, b] = await Promise.all([auth.accessToken(), auth.accessToken()]);
  assert.deepEqual([a, b, fetch.mock.callCount()], ['a', 'a', 1]);
});

/** A token answer for a sign-in whose nonce is `nonce()`, as a fetch stand-in. */
const answering = (nonce: () => string) => async () =>
  new Response(
    JSON.stringify({
      access_token: 'a',
      refresh_token: 'r',
      expires_in: 3600,
      scope: PLAN,
      id_token: idToken({ nonce: nonce() }),
    }),
  );

it('sends the same host id on every sign-in, made once and kept', async () => {
  let nonce = '';
  const { auth, fetch, opened, values } = setup(
    (state, n) => ((nonce = n), { state, code: 'c', client_id: 'oaiapp_x' }),
  );
  fetch.mock.mockImplementation(answering(() => nonce));
  await auth.signIn();
  const first = new URL(opened()).searchParams.get('ext_agent_host_id');
  await auth.signIn();
  assert.match(String(first), /^urn:uuid:/);
  assert.equal(new URL(opened()).searchParams.get('ext_agent_host_id'), first);
  assert.equal(values.chatgptHostId, first);
});

it('opens a later sign-in under the issued client, without registering again', async () => {
  let nonce = '';
  const { auth, fetch, opened } = setup((state, n) => ((nonce = n), { state, code: 'c', client_id: 'oaiapp_x' }));
  fetch.mock.mockImplementation(answering(() => nonce));
  assert.equal(new URL(await auth.signIn().then(opened)).searchParams.get('client_id'), 'dynamic_agent_client');
  await auth.signIn();
  const second = new URL(opened()).searchParams;
  assert.equal(second.get('client_id'), 'oaiapp_x');
  assert.equal(second.get('agent_name_hint'), null);
});

it('keeps the issued client when the exchange is refused, so the retry uses it', async () => {
  const { auth, fetch, opened, values } = setup((state) => ({ state, code: 'c', client_id: 'oaiapp_x' }));
  fetch.mock.mockImplementation(async () => new Response(JSON.stringify({ error: 'invalid_grant' }), { status: 400 }));
  await assert.rejects(auth.signIn(), /invalid_grant/);
  assert.equal(values.chatgptClientId, 'oaiapp_x');
  await assert.rejects(auth.signIn(), /invalid_grant/);
  assert.equal(new URL(opened()).searchParams.get('client_id'), 'oaiapp_x');
});

it('refuses a sign-in that comes back with a client other than the saved one, storing nothing', async () => {
  const { auth, fetch, saved, values } = setup((state) => ({ state, code: 'c', client_id: 'oaiapp_other' }));
  values.chatgptClientId = 'oaiapp_x';
  await assert.rejects(auth.signIn(), /did not match/);
  assert.equal(fetch.mock.callCount(), 0);
  assert.equal(saved(), null);
  assert.equal(values.chatgptClientId, 'oaiapp_x');
});

it('ignores a kept host id or client that is not text, registering afresh instead', async () => {
  let nonce = '';
  const { auth, fetch, opened, values } = setup(
    (state, n) => ((nonce = n), { state, code: 'c', client_id: 'oaiapp_x' }),
  );
  fetch.mock.mockImplementation(answering(() => nonce));
  Object.assign(values, { chatgptClientId: { x: 1 }, chatgptHostId: 42 });
  await auth.signIn();
  const params = new URL(opened()).searchParams;
  assert.equal(params.get('client_id'), 'dynamic_agent_client');
  assert.match(String(params.get('ext_agent_host_id')), /^urn:uuid:/);
  assert.equal(values.chatgptClientId, 'oaiapp_x');
});

it('says what OpenAI refused, so a failed sign-in can be told apart', async () => {
  const { auth, fetch } = setup((state) => ({ state, code: 'c', client_id: 'oaiapp_x' }));
  fetch.mock.mockImplementation(
    async () =>
      new Response(JSON.stringify({ error: 'invalid_grant', error_description: 'Code expired' }), { status: 400 }),
  );
  await assert.rejects(auth.signIn(), /invalid_grant: Code expired/);
});
