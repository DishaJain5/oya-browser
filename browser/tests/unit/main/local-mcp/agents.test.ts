/**
 * Adding Oya to a coding agent's config and taking it out again, in a scratch
 * home folder: only Oya's own entry is ever touched, and a file Oya cannot read
 * safely is refused, never overwritten.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { AGENTS, installedAgents } from '../../../../src/main/local-mcp/agents.ts';

const endpoint = { url: 'http://127.0.0.1:47321/mcp', token: 'oya_local_t' };

describe('coding agents', () => {
  let home = '';
  const realHome = process.env.HOME;
  beforeEach(() => {
    home = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-home-'));
    process.env.HOME = home;
    // Never let a test reach the person's real agent configs.
    assert.equal(os.homedir(), home, 'home folder must be the scratch one');
  });
  afterEach(() => {
    process.env.HOME = realHome;
    fs.rmSync(home, { recursive: true, force: true });
  });
  const cursorFile = () => path.join(home, '.cursor', 'mcp.json');

  it('lists Cursor once its folder exists, not before', () => {
    assert.equal(
      installedAgents().some((a) => a.id === 'cursor'),
      false,
    );
    fs.mkdirSync(path.join(home, '.cursor'));
    assert.deepEqual(
      installedAgents().find((a) => a.id === 'cursor'),
      { id: 'cursor', name: 'Cursor', connected: false },
    );
  });

  it('adds Oya to Cursor beside the servers already there, and removes only Oya', async () => {
    fs.mkdirSync(path.join(home, '.cursor'));
    fs.writeFileSync(cursorFile(), JSON.stringify({ mcpServers: { other: { url: 'x' } } }));
    await AGENTS.cursor.connect(endpoint);
    const added = JSON.parse(fs.readFileSync(cursorFile(), 'utf8')).mcpServers;
    assert.deepEqual(added['oya-browser'], { url: endpoint.url, headers: { Authorization: 'Bearer oya_local_t' } });
    assert.equal(AGENTS.cursor.connected(), true);
    assert.equal(fs.statSync(cursorFile()).mode & 0o777, 0o600, 'the token is readable by this user only');
    await AGENTS.cursor.disconnect();
    assert.deepEqual(JSON.parse(fs.readFileSync(cursorFile(), 'utf8')).mcpServers, { other: { url: 'x' } });
  });

  it('refuses a config with comments rather than overwrite it', async () => {
    fs.mkdirSync(path.join(home, '.cursor'));
    fs.writeFileSync(cursorFile(), '{ // mine\n "mcpServers": {} }');
    await assert.rejects(AGENTS.cursor.connect(endpoint), /by hand/);
    assert.match(fs.readFileSync(cursorFile(), 'utf8'), /\/\/ mine/);
  });

  it('adds Oya to Codex as a streamable HTTP server that sends the token', async () => {
    fs.mkdirSync(path.join(home, '.codex'));
    fs.writeFileSync(path.join(home, '.codex', 'config.toml'), 'model = "o3"\n');
    await AGENTS.codex.connect(endpoint);
    const toml = fs.readFileSync(path.join(home, '.codex', 'config.toml'), 'utf8');
    assert.match(toml, /^model = "o3"/);
    assert.match(toml, /\[mcp_servers\.oya-browser\]\nurl = "http:\/\/127\.0\.0\.1:47321\/mcp"\n/);
    assert.match(toml, /http_headers = \{ Authorization = "Bearer oya_local_t" \}/);
    assert.equal(AGENTS.codex.connected(), true);
  });
});
