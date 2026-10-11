/**
 * The ChatGPT credential at rest: one file in the app's data folder, sealed with
 * the OS keychain (safeStorage), readable by this user only. Refuses outright
 * where the keychain would store it in the clear, rather than keep a refresh
 * token any process could read.
 */
import fs from 'node:fs';
import path from 'node:path';
import type { Keychain } from '../workflow/draft-store.ts';
import { PRIVATE_FILE_MODE } from '../app/constants.ts';

/** What refusing says: the same reason the workflow drafts give. */
const INSECURE = 'Unlock your system keychain so Oya Browser can keep your ChatGPT sign-in safe.';

/** One JSON value, sealed in one file. */
export class SealedJson<T> {
  /** Where it lives. */
  private readonly file: string;
  /** The keychain that seals it. */
  private readonly safe: Keychain;

  /** Seals into `file` in `directory` with `safe`. */
  constructor(directory: string, file: string, safe: Keychain) {
    this.file = path.join(directory, file);
    this.safe = safe;
  }

  /** The value, or null when there is none or it no longer unseals. */
  load(): T | null {
    if (!fs.existsSync(this.file) || !this.secure()) return null;
    try {
      return JSON.parse(this.safe.decryptString(fs.readFileSync(this.file)));
    } catch {
      return null;
    }
  }

  /** Seals and writes the value, owner-only. */
  save(value: T): void {
    if (!this.secure()) throw Error(INSECURE);
    fs.mkdirSync(path.dirname(this.file), { recursive: true });
    fs.writeFileSync(this.file, this.safe.encryptString(JSON.stringify(value)), { mode: PRIVATE_FILE_MODE });
  }

  /** Forgets the value. */
  clear(): void {
    fs.rmSync(this.file, { force: true });
  }

  /** Whether the keychain really encrypts (not Linux's plain-text fallback). */
  private secure(): boolean {
    return this.safe.isEncryptionAvailable() && this.safe.getSelectedStorageBackend?.() !== 'basic_text';
  }
}
