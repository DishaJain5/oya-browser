/**
 * Unit tests for answering a connection request: the person sees which app asks,
 * may allow it only for a project they hold a role in, and every answer is a
 * single-use redirect back to the app carrying the app's state.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-mcp-oauth-connect-');
const { control, projectId } = await import('../../../../src/modules/control/service.ts');
const { allow, deny, describe: describeRequest } = await import('../../../../src/mcp/oauth/connect.ts');
const { openRequest, putClient } = await import('../../../../src/mcp/oauth/store.ts');
const { REQUEST_TTL_MS } = await import('../../../../src/mcp/oauth/constants.ts');
const { patchRow } = await import('../../support/control.ts');
const { inviteMember } = await import('../../../../src/modules/control/http/members.ts');
const { joinProject } = await import('../../../../src/modules/control/http/projects.ts');

const REDIRECT = 'https://claude.ai/api/mcp/auth_callback';
let n = 0;
let owner: string;
let key: string;
let id: string;

beforeEach(async () => {
  owner = `owner-${n}`;
  key = `key-${n}`;
  await control().project(key);
  await patchRow(control(), 'project', projectId(key), { ownerUser: owner });
  await putClient({ client_id: `client-${n}`, client_name: 'Claude', redirect_uris: [REDIRECT] } as any);
  id = await openRequest({ clientId: `client-${n++}`, redirectUri: REDIRECT, codeChallenge: 'c', state: 's1' });
});
afterEach(() => mock.timers.reset());

describe('connection requests', () => {
  it('names the app and the host its answer goes to', async () => {
    assert.deepEqual(await describeRequest(id), { client: 'Claude', redirectHost: 'claude.ai' });
  });

  it('allows a request with a code and the app state on its redirect', async () => {
    const url = new URL((await allow(id, owner, projectId(key))).redirect);
    assert.equal(url.origin + url.pathname, REDIRECT);
    assert.ok(url.searchParams.get('code'));
    assert.equal(url.searchParams.get('state'), 's1');
  });

  it('refuses a project the person holds no role in, and keeps the request open', async () => {
    await assert.rejects(allow(id, 'stranger', projectId(key)), { status: 404 });
    await assert.rejects(allow(id, owner, undefined), { status: 404 });
    assert.ok(await describeRequest(id));
  });

  it('refuses a viewer: every MCP call would refuse their connector anyway', async () => {
    const viewer = `viewer-${n}`;
    await joinProject(viewer, (await inviteMember(key, 'viewer')).code);
    await assert.rejects(allow(id, viewer, projectId(key)), { status: 403 });
    assert.ok(await describeRequest(id), 'the request stays open for a proper answer');
  });

  it('turns a request down with access_denied and the app state', async () => {
    const url = new URL((await deny(id)).redirect);
    assert.equal(url.searchParams.get('error'), 'access_denied');
    assert.equal(url.searchParams.get('state'), 's1');
  });

  it('answers a request only once', async () => {
    await deny(id);
    await assert.rejects(allow(id, owner, projectId(key)), { status: 410 });
    await assert.rejects(describeRequest(id), { status: 410 });
  });

  it('lets a request lapse after ten minutes', async () => {
    mock.timers.enable({ apis: ['Date'], now: Date.now() });
    const late = await openRequest({ clientId: 'x', redirectUri: REDIRECT, codeChallenge: 'c' });
    mock.timers.tick(REQUEST_TTL_MS + 1);
    await assert.rejects(allow(late, owner, projectId(key)), { status: 410 });
  });
});
