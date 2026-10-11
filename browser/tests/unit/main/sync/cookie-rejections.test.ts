/** Native cookie diagnostics disclose fixed rejection categories, never saved login attributes. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  checkCookieRejections,
  cookieRejectionCodes,
  cookieRejectionSummary,
} from '../../../../src/main/sync/cookie-rejections.ts';

test('counts only recognized native enum names without raw error text', () => {
  const reason = Error('secret=value private.example EXCLUDE_INVALID_PREFIX, EXCLUDE_USER_PREFERENCES');
  assert.deepEqual(cookieRejectionCodes(reason), ['EXCLUDE_INVALID_PREFIX', 'EXCLUDE_USER_PREFERENCES']);
});
test('unknown failures remain explicitly unclassified', () => {
  assert.deepEqual(cookieRejectionCodes(Error('storage unavailable secret=value')), ['unclassified']);
  assert.deepEqual(cookieRejectionCodes(Error('EXCLUDE_INVALID_PREFIX_EXTRA')), ['unclassified']);
});
test('aggregates each rejection and ignores successful writes', () => {
  assert.deepEqual(
    cookieRejectionSummary([
      { status: 'fulfilled', value: undefined },
      { status: 'rejected', reason: Error('EXCLUDE_INVALID_DOMAIN') },
      { status: 'rejected', reason: Error('EXCLUDE_INVALID_DOMAIN') },
      { status: 'rejected', reason: Error('unknown') },
    ]),
    { EXCLUDE_INVALID_DOMAIN: 2, unclassified: 1 },
  );
});

/** A rejection as stock Electron 44 words it: prose run straight into the enum names. */
const electronRefusal = (prose: string, codes: string) =>
  ({
    status: 'rejected',
    reason: Error(`Failed to set cookie - ${prose}${codes}, DO_NOT_WARN, NO_EXEMPTION`),
  }) as const;
test('reads the enum names Electron runs into its prose', () => {
  const { reason } = electronRefusal('The cookie contains ASCII control characters', 'EXCLUDE_DISALLOWED_CHARACTER');
  assert.deepEqual(cookieRejectionCodes(reason), ['EXCLUDE_DISALLOWED_CHARACTER']);
});
test('skips cookies Chromium can never store instead of failing the restore', () => {
  assert.doesNotThrow(() =>
    checkCookieRejections([
      { status: 'fulfilled', value: undefined },
      electronRefusal('The cookie contains ASCII control characters', 'EXCLUDE_DISALLOWED_CHARACTER'),
      electronRefusal('invalid Domain attribute.', 'EXCLUDE_DOMAIN_MISMATCH, EXCLUDE_INVALID_DOMAIN'),
      electronRefusal('size limit.', 'EXCLUDE_NAME_VALUE_PAIR_EXCEEDS_MAX_SIZE'),
      electronRefusal('not allowed to access Secure cookies.', 'EXCLUDE_SECURE_ONLY'),
    ]),
  );
});
test('a storage failure or an unknown reason still fails the restore', () => {
  for (const refusal of [
    electronRefusal('', 'EXCLUDE_FAILURE_TO_STORE'),
    electronRefusal('', 'EXCLUDE_INVALID_DOMAIN, EXCLUDE_NEW_REASON'),
    { status: 'rejected', reason: Error('EXCLUDE_INVALID_DOMAIN') } as const,
  ])
    assert.throws(() => checkCookieRejections([refusal]), /Cookie sync incomplete/);
});
