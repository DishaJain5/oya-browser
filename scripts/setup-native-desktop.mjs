/** Provision pinned desktop engines before signing; packaging retains its own native capability gate. */
import { createHash } from 'node:crypto';
import { createReadStream, appendFileSync, mkdtempSync, mkdirSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

/** Download bounds apply to each architecture independently. */
const DOWNLOAD_TIMEOUT_SECONDS = 600;
const CONNECT_TIMEOUT_SECONDS = 30;
const DOWNLOAD_RETRIES = 3;
/** Curl enforces HTTPS for the source and every redirect. */
const DOWNLOAD_OPTIONS = [
  '--fail',
  '--location',
  '--proto',
  '=https',
  '--proto-redir',
  '=https',
  '--retry',
  String(DOWNLOAD_RETRIES),
  '--connect-timeout',
  String(CONNECT_TIMEOUT_SECONDS),
  '--max-time',
  String(DOWNLOAD_TIMEOUT_SECONDS),
];
/** Universal macOS must supply both engines; Windows ships x64. */
const TARGETS = {
  darwin: [
    ['arm64', 'OYA_NATIVE_MACOS_ARM64'],
    ['x64', 'OYA_NATIVE_MACOS_X64'],
  ],
  win32: [['x64', 'OYA_NATIVE_WINDOWS_X64']],
};

/** Validate every pin before downloading any architecture or writing job environment variables. */
function settings(platform, env) {
  if (!Object.hasOwn(TARGETS, platform)) throw Error('Desktop engines require a macOS or Windows runner');
  if (!env.RUNNER_TEMP || !env.GITHUB_ENV) throw Error('RUNNER_TEMP and GITHUB_ENV must be set');
  return TARGETS[platform].map(([arch, name]) => pinnedTarget(platform, arch, name, env));
}

/** A repository variable must identify an HTTPS archive and its exact digest. */
function pinnedTarget(platform, arch, name, env) {
  const url = env[name + '_URL'] || '',
    sha256 = env[name + '_SHA256'] || '';
  const parsed = URL.canParse(url) ? new URL(url) : null;
  if (parsed?.protocol !== 'https:' || parsed.username || parsed.password || !/^[a-f\d]{64}$/i.test(sha256))
    throw Error(`Configure ${name}_URL and ${name}_SHA256 with a release-ready patched Oya distribution`);
  return { platform, arch, url, sha256: sha256.toLowerCase() };
}

/** Follow only HTTPS redirects and stop bounded download failures before extraction. */
function downloadArchive(url, archive) {
  execFileSync('curl', [...DOWNLOAD_OPTIONS, '--output', archive, url], { stdio: 'inherit' });
}

/** Hash as a stream so large native distributions do not consume the runner's memory. */
async function digest(archive) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(archive)) hash.update(chunk);
  return hash.digest('hex');
}

/** The pinned tar.gz contains the complete unpacked distribution at its root. */
function extractArchive(archive, directory) {
  execFileSync('tar', ['-xzf', archive, '-C', directory], { stdio: 'inherit' });
}

/** Reuse the release gate; a stock engine, wrong architecture, or failed probe is never exported. */
function verifyEngine(platform, arch, directory) {
  const require = createRequire(import.meta.url);
  require('../browser/build/native-distribution.cjs').verify(platform, arch, directory);
}

/** Verify archive integrity before extraction, then verify the actual extracted native runtime. */
async function installTarget(root, target, deps) {
  const name = `${target.platform}-${target.arch}`,
    archive = join(root, name + '.tar.gz');
  const directory = join(root, name);
  await deps.download(target.url, archive);
  if ((await digest(archive)) !== target.sha256) throw Error(`Native archive checksum mismatch: ${name}`);
  mkdirSync(directory);
  await deps.extract(archive, directory);
  await deps.verify(target.platform, target.arch, directory);
}

/** Publish the distribution root only after every required architecture passes its native probe. */
export async function installDesktopEngines({
  env = process.env,
  platform = process.platform,
  download = downloadArchive,
  extract = extractArchive,
  verify = verifyEngine,
} = {}) {
  const targets = settings(platform, env);
  const root = mkdtempSync(join(resolve(env.RUNNER_TEMP), 'oya-native-desktop-'));
  for (const target of targets) await installTarget(root, target, { download, extract, verify });
  appendFileSync(env.GITHUB_ENV, `OYA_NATIVE_DISTRIBUTIONS=${root}\n`);
  return root;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) await installDesktopEngines();
