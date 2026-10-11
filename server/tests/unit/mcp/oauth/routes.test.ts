/**
 * Unit tests for the connection routes: every one needs a signed-in person.
 * Allowing and turning down are covered through connect.ts.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-mcp-oauth-routes-');
const { connectRouter } = await import('../../../../src/mcp/oauth/routes.ts');
const { callRoute } = await import('../../support/agent.ts');

describe('connectRouter', () => {
  it('refuses a request without a token', async () => {
    for (const [method, url] of [
      ['GET', '/r1'],
      ['POST', '/r1/allow'],
      ['POST', '/r1/deny'],
    ]) {
      const res = await callRoute(connectRouter, { method, url });
      assert.equal(res.status, 401);
    }
  });
});
