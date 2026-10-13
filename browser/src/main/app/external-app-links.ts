/** Separate website navigation from desktop app links without rewriting private callback values. */
import { EXTERNAL_APP_URL_LIMIT, EXTERNAL_APP_LABEL_LIMIT } from './constants.ts';
/** These schemes belong to the browser, never an operating-system application handler. */
const INTERNAL = new Set([
  'http:',
  'https:',
  'about:',
  'data:',
  'blob:',
  'file:',
  'filesystem:',
  'javascript:',
  'vbscript:',
  'view-source:',
  'devtools:',
  'chrome:',
  'chrome-extension:',
  'electron:',
]);
/** Windows command and system-control handlers are not desktop application links. */
const BLOCKED = new Set(['shell:', 'cmd:', 'powershell:', 'ms-msdt:', 'search:', 'search-ms:', 'ms-settings:']);
/** Recognize protocols, but keep a typed localhost or dotted host with a port as a web address. */
export function externalProtocol(raw: string): string | null {
  const value = raw.trim();
  if (/^(localhost|[^\s/:]+\.[^\s/:]+):\d+(?:\/|$)/i.test(value)) return null;
  const scheme = value.match(/^[a-z][a-z\d+.-]*:/i)?.[0].toLowerCase();
  return scheme && !INTERNAL.has(scheme) ? scheme : null;
}
/** Preserve the exact URL; URL normalization can alter app authentication callbacks. */
export function externalAppLink(raw: string): string | null {
  const value = raw.trim(),
    scheme = externalProtocol(value);
  if (!scheme || BLOCKED.has(scheme) || value.length > EXTERNAL_APP_URL_LIMIT) return null;
  if (/[\p{Cc}\s]/u.test(value) || /%(?:0[0-9a-f]|1[0-9a-f]|7f)/i.test(value)) return null;
  const url = URL.parse(value);
  return url && !url.username && !url.password ? value : null;
}
/** Native handler labels remain plain, bounded text even if an application registered a misleading name. */
export const externalAppName = (name: string): string =>
  name
    .replace(/[\p{Cc}\p{Cf}]/gu, '')
    .trim()
    .slice(0, EXTERNAL_APP_LABEL_LIMIT);
/** Compatibility validator for callers that specifically need a Zoom meeting join. */
export function zoomMeetingLink(raw: string): string | null {
  if (raw.length > EXTERNAL_APP_URL_LIMIT || /[\p{Cc}\s]/u.test(raw)) return null;
  const url = URL.parse(raw);
  if (!url || url.protocol !== 'zoommtg:' || url.username || url.password || url.port || url.hash) return null;
  if (url.hostname !== 'zoom.us' && !url.hostname.endsWith('.zoom.us')) return null;
  return url.pathname === '/join' ? url.href : null;
}
