/**
 * One small JSON file in the user-data folder, owner-only. State that is not a
 * setting (site permissions, the last session's tabs) lives in its own file
 * rather than config.json, which is rewritten whole on every save and carries
 * the browsing library. The only place the stores it backs touch the filesystem.
 */
import fs from 'node:fs';
import { PRIVATE_FILE_MODE, JSON_INDENT } from './constants.ts';

/** A file holding one JSON value of type T. */
export class JsonFile<T> {
  /** Where the file is, asked on every read and write: the user-data folder is reset before the app is ready. */
  private readonly file: () => string;

  /** `file` names the path; it is asked lazily. */
  constructor(file: () => string) {
    this.file = file;
  }

  /** The value on disk, or null when the file is missing or unreadable: a bad file is treated as empty, never a crash. */
  read(): T | null {
    try {
      return JSON.parse(fs.readFileSync(this.file(), 'utf8')) as T;
    } catch {
      return null;
    }
  }

  /** Writes the value owner-only, whole: a temp file then a rename, so a crash mid-write leaves the last good file. A failed write keeps the running state, as config.json's does. */
  write(value: T): void {
    const file = this.file();
    try {
      fs.writeFileSync(`${file}.tmp`, JSON.stringify(value, null, JSON_INDENT), { mode: PRIVATE_FILE_MODE });
      fs.renameSync(`${file}.tmp`, file);
    } catch {}
  }
}
