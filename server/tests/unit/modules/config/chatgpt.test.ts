/**
 * Unit tests for Sign in with ChatGPT: the sign-in a self-hosted server starts
 * (dynamic registration, PKCE, loopback return, one host id), what the callback
 * accepts (single use, the right nonce, the plan permission), and the live token
 * a run gets: refreshed near expiry, one refresh at a time, rotated and saved.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-config-chatgpt-');
const { control } = await import('../../../../src/modules/control/service.ts');
const chatgpt = await import('../../../../src/modules/config/chatgpt.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');
const { CHATGPT_REFRESH_EARLY_MS } = await import('../../../../src/modules/config/constants.ts');
const { store } = await import('../../../../src/modules/config/store.ts');
const { DESKTOP_HELD } = await import('../../../../src/modules/config/chatgpt-credential.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');

const HOUR_S = 3600;
const PLAN = 'openid profile email offline_access resource.invoke chatgpt.tokens.use.direct';
let n = 0;
let key: string;

/** An unsigned ID token carrying `claims`. */
const idToken = (claims: object) =>
  ['{"alg":"none"}', JSON.stringify(claims), 'sig'].map((p) => Buffer.from(p).toString('base64url')).join('.');

/** The token endpoint's answer for a sign-in or refresh. */
const tokens = (over: object = {}) => ({
  access_token: `access-${n}`,
  refresh_token: `refresh-${n}`,
  expires_in: HOUR_S,
  scope: PLAN,
  ...over,
});

/** Stubs the token endpoint with `body` (status 200 unless given); returns the mock. */
const tokenEndpoint = (body: object, status = 200) =>
  mock.method(globalThis, 'fetch', async () => new Response(JSON.stringify(body), { status }));

/** Starts a sign-in; its authorize URL's parameters. */
const start = async () => new URL(await chatgpt.startLogin(key)).searchParams;

/** Signs the key in through the callback, the way ChatGPT returns to it. */
async function signIn(over: object = {}) {
  const params = await start();
  tokenEndpoint(tokens({ id_token: idToken({ nonce: params.get('nonce'), email: 'me@example.com' }), ...over }));
  return chatgpt.finishLogin({ state: params.get('state'), code: 'code-1', client_id: 'oaiapp_issued' });
}

beforeEach(async () => {
  key = `chatgpt-key-${n++}`;
  await control().project(key);
});
afterEach(() => {
  mock.restoreAll();
  mock.timers.reset();
});

describe('Sign in with ChatGPT', () => {
  it('starts a first sign-in as a dynamically registered agent host returning to this machine', async () => {
    const params = await start();
    assert.equal(params.get('client_id'), 'dynamic_agent_client');
    assert.equal(params.get('agent_name_hint'), 'Oya Browser');
    assert.equal(params.get('redirect_uri'), 'http://127.0.0.1:3100/auth/callback');
    assert.equal(params.get('code_challenge_method'), 'S256');
    assert.equal(params.get('resource'), 'https://api.openai.com/v1');
    assert.ok(params.get('scope')!.split(' ').includes('chatgpt.tokens.use.direct'));
    assert.match(params.get('ext_agent_host_id')!, /^urn:uuid:/);
    assert.equal((await start()).get('ext_agent_host_id'), params.get('ext_agent_host_id'));
  });

  it('is refused on a hosted deployment', async () => {
    process.env.STRIPE_SECRET_KEY = 'sk_test';
    try {
      await assert.rejects(chatgpt.startLogin(key), { status: 403 });
    } finally {
      delete process.env.STRIPE_SECRET_KEY;
    }
  });

  it('a completed sign-in puts the key on the ChatGPT plan with a live token', async () => {
    const { email } = await signIn();
    assert.equal(email, 'me@example.com');
    const form = new URLSearchParams(String((globalThis.fetch as any).mock.calls[0].arguments[1].body));
    assert.deepEqual([form.get('client_id'), form.get('grant_type')], ['oaiapp_issued', 'authorization_code']);
    assert.ok(form.get('code_verifier'));
    assert.equal(keyConfig.llmProviderOf(key), 'chatgpt');
    const llm = await chatgpt.resolveFresh(key);
    assert.deepEqual([llm.openaiKey, llm.baseUrl], [`access-${n}`, 'https://api.openai.com/v1/responses']);
  });

  it('sends the credential only to the plan, whatever llm_provider or openai_base_url say', async () => {
    await signIn();
    // Written straight to the store: set() would resolve the host, and the point is what resolveFresh does with it.
    const owner = fingerprint(key);
    store.set(owner, { ...store.get(owner), llm_provider: 'openai', openai_base_url: 'https://attacker.example/v1' });
    const llm = await chatgpt.resolveFresh(key);
    assert.equal(llm.baseUrl, 'https://api.openai.com/v1/responses');
    assert.ok(!llm.openaiKey.includes('refresh'));
  });

  it("drops the sign-in when the key moves to another provider, so it never leaves as that provider's key", async () => {
    await signIn();
    await keyConfig.set(key, { llm_provider: 'openai' });
    assert.equal(keyConfig.resolve(key).openaiKey, '');
    assert.equal((await chatgpt.resolveFresh(key)).openaiKey, '');
  });

  it('never spends a plan on a hosted deployment, even from a pasted credential', async () => {
    await signIn();
    process.env.STRIPE_SECRET_KEY = 'sk_test';
    try {
      assert.equal((await chatgpt.resolveFresh(key)).openaiKey, '');
    } finally {
      delete process.env.STRIPE_SECRET_KEY;
    }
  });

  it('a credential held on the desktop is relayed there, hosted or not, and never to a chosen host', async () => {
    await keyConfig.set(key, { llm_provider: 'chatgpt', openai_api_key: DESKTOP_HELD });
    process.env.STRIPE_SECRET_KEY = 'sk_test';
    try {
      const llm = await chatgpt.resolveFresh(key, 'b-1');
      assert.equal(llm.baseUrl, 'https://api.openai.com/v1/responses');
      assert.equal(typeof llm.transport, 'function');
      assert.equal(keyConfig.get(key).has_openai_key, true);
    } finally {
      delete process.env.STRIPE_SECRET_KEY;
    }
    await keyConfig.set(key, { llm_provider: 'openai' });
    assert.equal(keyConfig.resolve(key).openaiKey, '', "the marker never leaves as another provider's key");
  });

  it('reuses the issued client id on the next sign-in', async () => {
    await signIn();
    const params = await start();
    assert.equal(params.get('client_id'), 'oaiapp_issued');
    assert.equal(params.get('agent_name_hint'), null);
  });

  it('keeps the issued client id when the code exchange is refused, so the retry reuses it', async () => {
    const params = await start();
    tokenEndpoint({ error: 'invalid_grant' }, 400);
    const callback = { state: params.get('state'), code: 'c', client_id: 'oaiapp_issued' };
    await assert.rejects(chatgpt.finishLogin(callback), { message: /did not finish/ });
    const retry = await start();
    assert.equal(retry.get('client_id'), 'oaiapp_issued');
    assert.equal(retry.get('agent_name_hint'), null);
  });

  it('refuses a re-sign-in that comes back for another client, without exchanging its code', async () => {
    await signIn();
    const params = await start();
    const endpoint = tokenEndpoint(tokens());
    const callback = { state: params.get('state'), code: 'c', client_id: 'oaiapp_other' };
    await assert.rejects(chatgpt.finishLogin(callback), { status: 400, message: /did not match/ });
    assert.equal(endpoint.mock.callCount(), 0);
    assert.equal((await start()).get('client_id'), 'oaiapp_issued');
  });

  it('refuses a first sign-in that came back without an issued client id, without a doomed exchange', async () => {
    const params = await start();
    const endpoint = tokenEndpoint(tokens({ id_token: idToken({ nonce: params.get('nonce') }) }));
    await assert.rejects(chatgpt.finishLogin({ state: params.get('state'), code: 'c' }), {
      message: /did not finish the sign-in/,
    });
    assert.equal(endpoint.mock.callCount(), 0);
  });

  it('a sign-in the person cancelled names the key, so its waiting desktops can be told', async () => {
    const params = await start();
    const callback = chatgpt.finishLogin({ state: params.get('state'), error: 'access_denied' });
    await assert.rejects(callback, (err: any) => err.status === 403 && err.apiKey === key);
  });

  it('returns to the port the host publishes, not the container listens on', async () => {
    process.env.OYA_PUBLIC_PORT = '4000';
    try {
      assert.equal((await start()).get('redirect_uri'), 'http://127.0.0.1:4000/auth/callback');
    } finally {
      delete process.env.OYA_PUBLIC_PORT;
    }
  });

  it('answers a sign-in only once', async () => {
    const params = await start();
    tokenEndpoint(tokens({ id_token: idToken({ nonce: params.get('nonce') }) }));
    const callback = { state: params.get('state'), code: 'c', client_id: 'oaiapp_issued' };
    await chatgpt.finishLogin(callback);
    await assert.rejects(chatgpt.finishLogin(callback), { status: 410 });
  });

  it('refuses an ID token for another sign-in, and tokens without the plan permission', async () => {
    await assert.rejects(signIn({ id_token: idToken({ nonce: 'someone-else' }) }), { status: 400 });
    await assert.rejects(signIn({ scope: 'openid profile email' }), { status: 403 });
    assert.notEqual(keyConfig.llmProviderOf(key), 'chatgpt');
  });

  it('refreshes near expiry, once for callers at the same time, and keeps the rotated token', async () => {
    mock.timers.enable({ apis: ['Date'], now: Date.now() });
    await signIn();
    mock.timers.tick(HOUR_S * 1000 - CHATGPT_REFRESH_EARLY_MS + 1);
    const endpoint = tokenEndpoint({ access_token: 'access-new', refresh_token: 'refresh-new', expires_in: HOUR_S });
    const [a, b] = await Promise.all([chatgpt.resolveFresh(key), chatgpt.resolveFresh(key)]);
    assert.deepEqual([a.openaiKey, b.openaiKey], ['access-new', 'access-new']);
    assert.equal(endpoint.mock.callCount(), 1);
    const form = new URLSearchParams(String(endpoint.mock.calls[0].arguments[1].body));
    assert.deepEqual([form.get('grant_type'), form.get('client_id')], ['refresh_token', 'oaiapp_issued']);
    assert.equal(chatgpt.credentialOf(keyConfig.resolve(key).openaiKey)!.refresh_token, 'refresh-new');
  });

  it('asks for a new sign-in when a refresh is refused', async () => {
    mock.timers.enable({ apis: ['Date'], now: Date.now() });
    await signIn();
    mock.timers.tick(HOUR_S * 1000);
    tokenEndpoint({ error: 'invalid_grant' }, 400);
    await assert.rejects(chatgpt.resolveFresh(key), { status: 422, message: /sign in with ChatGPT again/ });
  });
});
