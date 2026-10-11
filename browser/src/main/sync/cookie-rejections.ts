/** Summarize native cookie rejections using fixed labels without exposing saved login data. */
import { COOKIE_REJECTION_CODES, COOKIE_LOCAL_CONFLICT_CODES, COOKIE_UNSTORABLE_CODES } from './constants.ts';
/**
 * Only native enum names on the fixed allowlist can appear in diagnostics. Electron
 * runs its prose straight into the first name ("charactersEXCLUDE_..."), so a name
 * may start mid-word; it must still end at a word boundary.
 */
export function cookieRejectionCodes(reason: unknown): string[] {
  const message = reason instanceof Error ? reason.message : '';
  const found = [...new Set(message.match(/EXCLUDE_[A-Z_]+\b/g) || [])];
  return found.length ? found.map(knownCookieRejection) : ['unclassified'];
}
/** Count each reported native reason without including source errors or cookie attributes. */
export function cookieRejectionSummary(results: PromiseSettledResult<unknown>[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const result of results) {
    if (result.status !== 'rejected') continue;
    for (const code of cookieRejectionCodes(result.reason)) counts[code] = (counts[code] || 0) + 1;
  }
  return counts;
}

/** Unknown native enums never become a successful local-conflict outcome. */
function knownCookieRejection(code: string): string {
  return COOKIE_REJECTION_CODES.includes(code as never) ? code : 'unclassified';
}
/** A native set-cookie refusal whose every reason is in `codes`; anything else is not. */
function refusedOnlyFor(result: PromiseSettledResult<unknown>, codes: readonly string[]): boolean {
  if (result.status !== 'rejected' || !(result.reason instanceof Error)) return false;
  if (!result.reason.message.startsWith('Failed to set cookie - ')) return false;
  return cookieRejectionCodes(result.reason).every((code) => codes.includes(code));
}
/** Only explicit native overwrite protection is safe to treat as keeping a stronger local login. */
const preservedLocalCookie = (result: PromiseSettledResult<unknown>) =>
  refusedOnlyFor(result, COOKIE_LOCAL_CONFLICT_CODES);
/** A cookie Chromium can never store as given: skipped, since no retry would take it. */
const unstorableCookie = (result: PromiseSettledResult<unknown>) => refusedOnlyFor(result, COOKIE_UNSTORABLE_CODES);
/** Logs what was set aside, by category, when anything was. */
function noteSkipped(label: string, skipped: PromiseSettledResult<unknown>[]): void {
  if (skipped.length) console.warn(label, JSON.stringify(cookieRejectionSummary(skipped)));
}
/** Keep native overwrite protections and skip unstorable cookies; storage, unknown and malformed failures stay fatal. */
export function checkCookieRejections(results: PromiseSettledResult<unknown>[]): void {
  noteSkipped('[oya] Cookie sync kept stronger local cookies:', results.filter(preservedLocalCookie));
  noteSkipped('[oya] Cookie sync skipped cookies Chromium cannot store:', results.filter(unstorableCookie));
  const failures = results.filter((r) => r.status === 'rejected' && !preservedLocalCookie(r) && !unstorableCookie(r));
  if (failures.length) throw Error('Cookie sync incomplete: ' + JSON.stringify(cookieRejectionSummary(failures)));
}
