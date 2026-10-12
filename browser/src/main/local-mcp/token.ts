/**
 * The MCP port's bearer token: made once, kept sealed with the OS keychain in
 * the app's data folder, so agents configured with it keep working across
 * restarts. Without a real keychain it lives only for this run.
 */
import { randomBytes } from 'node:crypto';
import type { AppServices } from '../app/services.ts';
import { SealedJson } from '../chatgpt/credential-store.ts';
import { MCP_TOKEN_BYTES, MCP_TOKEN_FILE } from './constants.ts';

/** The app's data folder and keychain. */
type Deps = Pick<AppServices, 'electron'>;

/** The token on disk. */
interface Stored {
  /** The bearer token. */
  token: string;
}

/** The saved token, or a new one saved now. */
export function ensureToken({ electron }: Deps): string {
  const store = new SealedJson<Stored>(electron.app.getPath('userData'), MCP_TOKEN_FILE, electron.safeStorage);
  const saved = store.load()?.token;
  if (saved) return saved;
  const token = `oya_local_${randomBytes(MCP_TOKEN_BYTES).toString('base64url')}`;
  keep(store, token);
  return token;
}

/** Saves the token; without a real keychain it stays for this run only. */
function keep(store: SealedJson<Stored>, token: string): void {
  try {
    store.save({ token });
  } catch (e) {
    console.warn('[oya] MCP token kept for this run only:', (e as Error).message);
  }
}
