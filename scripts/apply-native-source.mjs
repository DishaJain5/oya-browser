/** Apply the pinned source overlay only to byte-identical Electron-patched baseline files. */
import { readFileSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';

/** Exact file bytes bind the overlay to the upstream patch hooks, independently of commit timestamps. */
const digest = (file) => createHash('sha256').update(readFileSync(file)).digest('hex');

/** Refuse a baseline mismatch or overwritten new file before mutating any source repository. */
function validateRepository(source, overlay, repository) {
  const directory = resolve(source, repository.path),
    patch = resolve(overlay, repository.patch);
  if (digest(patch) !== repository.sha256) throw Error(`Source patch digest mismatch: ${repository.path}`);
  for (const [file, expected] of Object.entries(repository.baselineFiles))
    if (digest(join(directory, file)) !== expected) throw Error(`Source baseline mismatch: ${repository.path}/${file}`);
  for (const file of repository.newFiles)
    if (existsSync(join(directory, file))) throw Error(`Source overlay would overwrite ${repository.path}/${file}`);
  execFileSync('git', ['apply', '--check', patch], { cwd: directory, stdio: 'inherit' });
}

/** Validate all repositories before applying the first patch; failure never triggers a fuzzy fallback. */
export function applyNativeSource(source, overlay) {
  const manifest = JSON.parse(readFileSync(join(overlay, 'manifest.json'), 'utf8'));
  for (const repository of manifest.repositories) validateRepository(source, overlay, repository);
  for (const repository of manifest.repositories)
    execFileSync('git', ['apply', resolve(overlay, repository.patch)], {
      cwd: resolve(source, repository.path),
      stdio: 'inherit',
    });
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href)
  applyNativeSource(process.argv[2], process.argv[3]);
