/**
 * Saved passwords from a password manager's CSV export (the Passwords app, Safari,
 * Chrome, Firefox, 1Password, Bitwarden) into the server's sealed sign-in store for
 * the persona this browser runs as, so the agent can sign in where a session has
 * expired. macOS keeps the Passwords app's keychain to Apple's own apps, so an
 * export is the only way in. The file is read once and nothing of it is kept here.
 */
import { readFile, stat } from 'node:fs/promises';
import type { OpenDialogOptions } from 'electron';
import type { AppServices } from '../app/services.ts';
import { ServerApi, readApiAnswer } from '../connection/server-api.ts';
import type { PasswordImportResult } from '../../shared/ipc.ts';
import { MAX_PASSWORD_CSV_BYTES } from './constants.ts';

/** What an import uses: the open dialog, its parent window, and the server. */
type Deps = Pick<AppServices, 'electron' | 'shell' | 'config' | 'socket'>;

/** One saved login as the server takes it. */
export interface Login {
  /** The site's address. */
  url: string;
  /** The account name. */
  username: string;
  /** The password. */
  password: string;
}

/** The names each column goes by across exporters, lower-cased. */
const COLUMNS: Record<keyof Login, readonly string[]> = {
  url: ['url', 'website', 'web site', 'login_uri', 'login uri'],
  username: ['username', 'user name', 'login_username', 'login name', 'email'],
  password: ['password', 'login_password'],
};

/** A CSV token: a separator, a line break, a quoted field, or a run of plain text. */
const CSV_TOKEN = /,|\r?\n|"(?:[^"]|"")*"|[^,\r\n"]+/g;

/** The open dialog: where the Passwords app puts its export is the person's choice. */
const CHOOSE: OpenDialogOptions = {
  title: 'Import passwords',
  buttonLabel: 'Import',
  message: 'Choose a passwords CSV. In the Passwords app: File > Export All Passwords.',
  properties: ['openFile'],
  filters: [{ name: 'Passwords (CSV)', extensions: ['csv'] }],
};

/** RFC 4180 rows: a quoted field may hold commas, doubled quotes and line breaks. */
export function csvRows(text: string): string[][] {
  const rows: string[][] = [[]];
  let field = '';
  for (const [token] of `${text.replace(/^\uFEFF/, '')}\n`.matchAll(CSV_TOKEN)) field = take(rows, field, token);
  return rows.filter((row) => row.some(Boolean));
}

/** Adds one token to the rows read so far; answers the field still being read. */
function take(rows: string[][], field: string, token: string): string {
  if (token !== ',' && !token.endsWith('\n')) return field + unquoted(token);
  rows.at(-1)!.push(field);
  if (token.endsWith('\n')) rows.push([]);
  return '';
}

/** A field's text: a quoted one loses its quotes and has its doubled quotes halved. */
const unquoted = (token: string): string => (token.startsWith('"') ? token.slice(1, -1).replaceAll('""', '"') : token);

/** The logins in an export, by its header; throws when it has no address or password column. */
export function loginsFrom(text: string): Login[] {
  const [header = [], ...rows] = csvRows(text);
  const names = header.map((name) => name.trim().toLowerCase());
  const at = (column: keyof Login) => names.findIndex((name) => COLUMNS[column].includes(name));
  const [url, username, password] = [at('url'), at('username'), at('password')];
  if (url < 0 || password < 0) throw new Error('This file has no website and password columns to import.');
  return rows
    .map((row) => ({ url: row[url] ?? '', username: row[username] ?? '', password: row[password] ?? '' }))
    .filter((login) => login.password);
}

/** The export's text, refused when it is too big to be one. */
async function readExport(file: string): Promise<string> {
  if ((await stat(file)).size > MAX_PASSWORD_CSV_BYTES)
    throw new Error('This file is too large to be a passwords export.');
  return readFile(file, 'utf8');
}

/** Imports a passwords export the person chooses. */
export class PasswordImport {
  /** The dialog and the window. */
  private readonly deps: Deps;
  /** The server, as this browser. */
  private readonly api: ServerApi;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.api = new ServerApi(deps);
  }

  /** Asks for the file and sends its logins: the counts, or null when the person cancelled. */
  async run(): Promise<PasswordImportResult | null> {
    if (!this.api.canCall()) throw new Error('Connect to your Oya server to import passwords.');
    const file = await this.choose();
    if (!file) return null;
    const entries = loginsFrom(await readExport(file));
    const res = await this.api.postToBrowser('credentials/import', { entries });
    return (await readApiAnswer(res)) as PasswordImportResult;
  }

  /** The chosen file, or null. */
  private async choose(): Promise<string | null> {
    const { dialog } = this.deps.electron;
    const win = this.deps.shell.window;
    const picked = win ? await dialog.showOpenDialog(win, CHOOSE) : await dialog.showOpenDialog(CHOOSE);
    return picked.canceled ? null : (picked.filePaths[0] ?? null);
  }
}
