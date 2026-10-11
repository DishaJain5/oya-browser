/**
 * Unit tests for the MCP OAuth provider: a sign-in goes to the console page, an
 * allowed code buys one credential for the chosen project, once, within a
 * minute, for the client and redirect it was issued to; revoking the token ends
 * it, and refresh tokens are not issued.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-mcp-oauth-provider-');
const { control, projectId } = await import('../../../../src/modules/control/service.ts');
const { authenticateToken } = await import('../../../../src/modules/auth/service.ts');
const { provider } = await import('../../../../src/mcp/oauth/provider.ts');
const { allow } = await import('../../../../src/mcp/oauth/connect.ts');
const { CODE_TTL_MS } = await import('../../../../src/mcp/oauth/constants.ts');
const { patchRow } = await import('../../support/control.ts');

const REDIRECT = 'https://claude.ai/api/mcp/auth_callback';
let n = 0;
let owner: string;
let key: string;

/** Registers a client the way the SDK's /register handler does. */
const register = (name = 'Claude') =>
  provider.clientsStore.registerClient!({
    client_id: `client-${n++}`,
    client_name: name,
    redirect_uris: [REDIRECT],
  } as any);

/** Starts a sign-in for `client`; the request id from the console link it redirects to. */
async function start(client) {
  let location = '';
  const res = { redirect: (_status: number, url: string) => (location = url) };
  await provider.authorize(client, { redirectUri: REDIRECT, codeChallenge: 'challenge', state: 's1' }, res as any);
  return { location, id: new URL(location).searchParams.get('request') as string };
}

/** A code the owner allowed for their project. */
async function codeFor(client) {
  const { redirect } = await allow((await start(client)).id, owner, projectId(key));
  return new URL(redirect).searchParams.get('code') as string;
}

beforeEach(async () => {
  owner = `owner-${n}`;
  key = `key-${n++}`;
  await control().project(key);
  await patchRow(control(), 'project', projectId(key), { ownerUser: owner });
});
afterEach(() => mock.timers.reset());

describe('MCP OAuth provider', () => {
  it('sends the person to the console connect page with a request id', async () => {
    const { location, id } = await start(await register());
    assert.match(location, /\/connect\?request=/);
    assert.ok(id.length > 20);
  });

  it('exchanges a code for a credential on the project the person chose', async () => {
    const client = await register('Claude');
    const tokens = await provider.exchangeAuthorizationCode(client, await codeFor(client), 'v', REDIRECT);
    const principal = await authenticateToken(tokens.access_token);
    assert.equal(principal.key, key);
    // The owner administers the project; the connector only ever operates it.
    assert.equal(principal.role, 'operator');
    assert.equal(tokens.token_type, 'bearer');
  });

  it('reads the PKCE challenge without spending the code', async () => {
    const client = await register();
    const code = await codeFor(client);
    assert.equal(await provider.challengeForAuthorizationCode(client, code), 'challenge');
    assert.ok((await provider.exchangeAuthorizationCode(client, code)).access_token);
  });

  it('spends a code once', async () => {
    const client = await register();
    const code = await codeFor(client);
    await provider.exchangeAuthorizationCode(client, code);
    await assert.rejects(provider.exchangeAuthorizationCode(client, code), { name: 'InvalidGrantError' });
  });

  it('refuses a code after a minute', async () => {
    mock.timers.enable({ apis: ['Date'], now: Date.now() });
    const client = await register();
    const code = await codeFor(client);
    mock.timers.tick(CODE_TTL_MS + 1);
    await assert.rejects(provider.exchangeAuthorizationCode(client, code), { name: 'InvalidGrantError' });
  });

  it('refuses a code issued to another client', async () => {
    const code = await codeFor(await register());
    await assert.rejects(provider.exchangeAuthorizationCode(await register(), code), { name: 'InvalidGrantError' });
  });

  it('refuses a code sent with a different redirect URI', async () => {
    const client = await register();
    const exchange = provider.exchangeAuthorizationCode(client, await codeFor(client), 'v', 'https://evil.example/cb');
    await assert.rejects(exchange, { name: 'InvalidGrantError' });
  });

  it('revoking the token ends the credential', async () => {
    const client = await register();
    const { access_token } = await provider.exchangeAuthorizationCode(client, await codeFor(client));
    await provider.revokeToken!(client, { token: access_token });
    await assert.rejects(authenticateToken(access_token));
    await assert.rejects(provider.verifyAccessToken(access_token), { name: 'InvalidTokenError' });
  });

  it('a code for a project deleted since it was allowed is an invalid grant, not a server error', async () => {
    const client = await register();
    const code = await codeFor(client);
    await patchRow(control(), 'project', projectId(key), { deletedAt: 1 });
    await assert.rejects(provider.exchangeAuthorizationCode(client, code), { name: 'InvalidGrantError' });
  });

  it('refuses a registration with too many redirect URIs or an overlong name', async () => {
    const many = Array.from({ length: 6 }, (_, i) => `https://a.example/cb${i}`);
    const registerWith = (over: object) =>
      provider.clientsStore.registerClient!({ client_id: 'x', redirect_uris: [REDIRECT], ...over } as any);
    await assert.rejects(registerWith({ redirect_uris: many }), { name: 'InvalidClientMetadataError' });
    await assert.rejects(registerWith({ client_name: 'n'.repeat(81) }), { name: 'InvalidClientMetadataError' });
  });

  it('issues no refresh tokens', async () => {
    await assert.rejects(provider.exchangeRefreshToken(await register(), 'r'), { name: 'InvalidGrantError' });
  });
});
