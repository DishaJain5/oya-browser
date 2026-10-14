/**
 * Where a download is saved: the person's Downloads folder, under the name the
 * site suggested, made free with " (2)", " (3)"… so nothing is overwritten. The
 * name is claimed by creating the file, so two downloads never get the same one.
 */
import { closeSync, existsSync, openSync } from 'node:fs';
import { basename, extname, join } from 'node:path';
import { NAME_ATTEMPTS } from './constants.ts';

/** A site's suggested name, kept to a plain file name: no folders, never empty. */
function plainName(suggested: string): string {
  const name = basename(suggested.replace(/[\\\\/]/g, '_')).trim();
  return name && name !== '.' && name !== '..' ? name : 'download';
}

/** The `n`th name tried for `name`: itself, then "stem (2).ext", "stem (3).ext"… */
function nthName(name: string, n: number): string {
  const ext = extname(name);
  return n === 1 ? name : `${name.slice(0, name.length - ext.length)} (${n})${ext}`;
}

/** Creates `path` when nothing is there; false when something is. */
function claim(path: string): boolean {
  try {
    return (closeSync(openSync(path, 'wx')), true);
  } catch {
    return false;
  }
}

/** Claims and answers a free path for `suggested` in `folder`. */
export function claimPath(folder: string, suggested: string): string {
  const name = plainName(suggested);
  for (let n = 1; n <= NAME_ATTEMPTS; n++)
    if (claim(join(folder, nthName(name, n)))) return join(folder, nthName(name, n));
  throw Error('No free file name for this download');
}

/** Whether a saved download's file is still there. */
export const fileExists = (path: string): boolean => existsSync(path);
