/**
 * Reading another browser's SQLite files (cookies, history, bookmarks) by the system's
 * sqlite3, from a copy taken with its write-ahead log: a running browser keeps its
 * newest rows there and holds a lock on the original.
 */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { copyIfPresent } from './copy.ts';
import { SQLITE_MAX_BUFFER } from './constants.ts';

/** The rows `sql` answers from a copy of `file`, or none when there is no such file. */
export function sqliteRows<T>(file: string, sql: string): T[] {
  if (!fs.existsSync(file)) return [];
  const copy = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'oya-db-')), 'db.sqlite');
  for (const suffix of ['', '-wal']) copyIfPresent(file + suffix, copy + suffix);
  try {
    return sqliteJson<T>(copy, sql);
  } finally {
    fs.rmSync(path.dirname(copy), { recursive: true, force: true });
  }
}

/** The rows `sql` answers from `file`, read-only. */
function sqliteJson<T>(file: string, sql: string): T[] {
  const opts = { encoding: 'utf8', maxBuffer: SQLITE_MAX_BUFFER } as const;
  return JSON.parse(execFileSync('sqlite3', ['-json', '-readonly', file, sql], opts) || '[]') as T[];
}
