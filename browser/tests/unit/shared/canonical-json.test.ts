/**
 * Unit tests for order-independent JSON: the same content gives the same text
 * whichever order its fields arrived in; different content never does.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { canonicalJson } from '../../../src/shared/canonical-json.ts';

it('gives a persona read from disk and the same one from the server the same text', () => {
  const disk = { id: 'm-1', screen: { width: 1512, height: 982 }, fonts: { available: ['a', 'b'] } };
  const server = { fonts: { available: ['a', 'b'] }, screen: { height: 982, width: 1512 }, id: 'm-1' };
  assert.equal(canonicalJson({ profile: disk }), canonicalJson({ profile: server }));
});

it('still tells a real change apart, array order included', () => {
  assert.notEqual(canonicalJson({ a: 1 }), canonicalJson({ a: 2 }));
  assert.notEqual(canonicalJson({ fonts: ['a', 'b'] }), canonicalJson({ fonts: ['b', 'a'] }));
});
