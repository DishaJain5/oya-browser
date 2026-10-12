/**
 * The coding agents on this computer that can use Oya's MCP port: which are
 * installed, which already list Oya, and how to add or remove Oya from each.
 * Oya only ever touches its own entry ("oya-browser"), and only when the person
 * clicks Connect or Disconnect. An agent's own CLI makes the change where it has
 * one, so its config file is never written underneath it.
 */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { MCP_SERVER_NAME } from './constants.ts';
import { PRIVATE_FILE_MODE } from '../app/constants.ts';

/** Spaces JSON configs are indented with, as the agents write them. */
const JSON_INDENT = 2;

/** Runs a program, its output ignored. */
const run = promisify(execFile);

/** Where Oya's port is and the token it wants. */
export interface Endpoint {
  /** The MCP URL. */
  url: string;
  /** The bearer token. */
  token: string;
}

/** One agent Oya can be added to. */
interface AgentTarget {
  /** Its name, as the footer shows it. */
  name: string;
  /** Whether it is on this computer. */
  installed(): boolean;
  /** Whether its config lists Oya. */
  connected(): boolean;
  /** Adds Oya to its config. */
  connect(endpoint: Endpoint): Promise<void>;
  /** Removes Oya from its config. */
  disconnect(): Promise<void>;
}

/** An agent as the footer lists it. */
export interface AgentStatus {
  /** Its id. */
  id: string;
  /** Its name. */
  name: string;
  /** Whether its config lists Oya. */
  connected: boolean;
}

/** A path under the home folder. */
const home = (...parts: string[]) => path.join(os.homedir(), ...parts);

/** Folders a CLI installed per user or by Homebrew lives in; an app opened from Finder has none of them on PATH. */
const binDirs = () => [
  ...[home('.local/bin'), home('.claude/local'), home('.npm-global/bin')],
  ...['/opt/homebrew/bin', '/usr/local/bin'],
];

/** The full path of CLI `name`, or null. */
const binary = (name: string): string | null =>
  binDirs()
    .map((dir) => path.join(dir, name))
    .find((file) => fs.existsSync(file)) ?? null;

/** Runs CLI `name` with the bin folders on PATH, so a Node-based CLI finds node. */
function cli(name: string, args: string[]): Promise<unknown> {
  const file = binary(name);
  if (!file) return Promise.reject(new Error(`${name} is not installed`));
  return run(file, args, { env: { ...process.env, PATH: [...binDirs(), process.env.PATH].join(':') } });
}

/** A JSON file's contents, {} when absent; a file that is not plain JSON is refused rather than overwritten. */
function readJson(file: string): Record<string, Record<string, unknown>> {
  if (!fs.existsSync(file)) return {};
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch {
    throw new Error(`${file} has comments or is not plain JSON; add Oya there by hand.`);
  }
}

/** Sets (or, with null, removes) Oya's entry under `key` in JSON config `file`. */
function writeJsonEntry(file: string, key: string, entry: object | null): void {
  const config = readJson(file);
  const servers = { ...(config[key] ?? {}) };
  if (entry) servers[MCP_SERVER_NAME] = entry;
  else delete servers[MCP_SERVER_NAME];
  writePrivate(file, JSON.stringify({ ...config, [key]: servers }, null, JSON_INDENT) + '\n');
}

/** Writes (or with `append`, adds to) a config that now holds Oya's token, readable by this user only. */
function writePrivate(file: string, text: string, append = false): void {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  (append ? fs.appendFileSync : fs.writeFileSync)(file, text, { mode: PRIVATE_FILE_MODE });
  fs.chmodSync(file, PRIVATE_FILE_MODE); // a file that existed before keeps its old mode otherwise
}

/** An agent whose MCP servers sit in JSON file `file()` (resolved when used) under `key`, each entry shaped by `entry`. */
function jsonAgent(name: string, file: () => string, key: string, entry: (e: Endpoint) => object): AgentTarget {
  return {
    name,
    installed: () => fs.existsSync(path.dirname(file())),
    connected: () => Object.hasOwn(readJsonQuietly(file())[key] ?? {}, MCP_SERVER_NAME),
    connect: async (e) => writeJsonEntry(file(), key, entry(e)),
    disconnect: async () => writeJsonEntry(file(), key, null),
  };
}

/** A JSON file's contents, {} when it cannot be read. */
function readJsonQuietly(file: string): Record<string, Record<string, unknown>> {
  try {
    return readJson(file);
  } catch {
    return {};
  }
}

/** The header every entry sends. */
const auth = ({ token }: Endpoint) => ({ Authorization: `Bearer ${token}` });

/** Claude Code's `mcp add` for Oya's port, at user scope, sending the token. */
const claudeAdd = ({ url, token }: Endpoint) => [
  ...['mcp', 'add', '--scope', 'user', '--transport', 'http', MCP_SERVER_NAME, url],
  ...['--header', `Authorization: Bearer ${token}`],
];

/** Claude Code: its own CLI adds and removes the server, at user scope so every project sees it. */
const CLAUDE_CODE: AgentTarget = {
  name: 'Claude Code',
  installed: () => binary('claude') !== null,
  connected: () => Object.hasOwn(readJsonQuietly(home('.claude.json')).mcpServers ?? {}, MCP_SERVER_NAME),
  connect: async (e) => {
    await cli('claude', ['mcp', 'remove', MCP_SERVER_NAME, '--scope', 'user']).catch(() => {});
    await cli('claude', claudeAdd(e));
  },
  disconnect: async () => void (await cli('claude', ['mcp', 'remove', MCP_SERVER_NAME, '--scope', 'user'])),
};

/** Codex's config, where Oya's section goes. */
const codexConfig = () => home('.codex', 'config.toml');

/** Oya's section in Codex's TOML config. */
const codexSection = (e: Endpoint) =>
  `\n[mcp_servers.${MCP_SERVER_NAME}]\nurl = ${JSON.stringify(e.url)}\nhttp_headers = { Authorization = ${JSON.stringify(`Bearer ${e.token}`)} }\n`;

/** Codex: its CLI cannot set a header, so Oya's section is appended after its CLI removes any older one. */
const CODEX: AgentTarget = {
  name: 'Codex',
  installed: () => binary('codex') !== null || fs.existsSync(home('.codex')),
  connected: () =>
    fs.existsSync(codexConfig()) && fs.readFileSync(codexConfig(), 'utf8').includes(`[mcp_servers.${MCP_SERVER_NAME}]`),
  connect: async (e) => {
    if (CODEX.connected()) await CODEX.disconnect();
    writePrivate(codexConfig(), codexSection(e), true);
  },
  disconnect: async () => void (await cli('codex', ['mcp', 'remove', MCP_SERVER_NAME])),
};

/** Cursor's entry for Oya's port. */
const cursorEntry = (e: Endpoint): object => ({ url: e.url, headers: auth(e) });

/** VS Code's entry for Oya's port. */
const vscodeEntry = (e: Endpoint): object => ({ type: 'http', url: e.url, headers: auth(e) });

/** Every agent Oya knows how to connect, by id. */
export const AGENTS: Readonly<Record<string, AgentTarget>> = {
  'claude-code': CLAUDE_CODE,
  codex: CODEX,
  cursor: jsonAgent('Cursor', () => home('.cursor', 'mcp.json'), 'mcpServers', cursorEntry),
  vscode: jsonAgent('VS Code', () => home('Library/Application Support/Code/User/mcp.json'), 'servers', vscodeEntry),
};

/** The installed agents, and whether each lists Oya. */
export function installedAgents(): AgentStatus[] {
  return Object.entries(AGENTS)
    .filter(([, agent]) => agent.installed())
    .map(([id, agent]) => ({ id, name: agent.name, connected: agent.connected() }));
}
