/** Private-cookie capabilities require explicit context ownership and implemented attribute semantics. */
import { cookieBatch, cookieURL, cookieString } from '../native-cookies/index.ts';
import type { NativeCommand } from './types.ts';
/** Standard Storage commands and one explicit URL/name deletion extension. */
export const COOKIE_PARAMS: Record<string, readonly string[]> = {
  'Storage.getCookies': ['browserContextId'],
  'Storage.setCookies': ['browserContextId', 'cookies'],
  'Storage.clearCookies': ['browserContextId'],
  'Oya.deleteCookie': ['browserContextId', 'url', 'name'],
};
/** A missing context means the browser's own profile (the persona's), as in Chrome. */
export function validateCookies({ method, params }: NativeCommand): void {
  if (!Object.hasOwn(COOKIE_PARAMS, method)) return;
  optionalContext(params.browserContextId);
  if (method === 'Storage.setCookies') cookieBatch(params.cookies);
  if (method === 'Oya.deleteCookie') {
    cookieURL(params.url);
    cookieString(params.name, true);
  }
}

/** A context is omitted (the browser's own profile) or a nonempty id. */
function optionalContext(context: unknown): void {
  if (context !== undefined && (typeof context !== 'string' || !context))
    throw Error('An owned browserContextId is required');
}
