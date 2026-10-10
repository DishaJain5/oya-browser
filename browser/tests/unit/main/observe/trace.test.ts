/** Trace cursors, exact-tab scope, revocation and bounded memory are externally observable contracts. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { Traces } from '../../../../src/main/observe/trace.ts';
import { TRACE_EVENTS_MAX, TRACE_MAX } from '../../../../src/main/observe/constants.ts';
it('keeps console and network in one chronological cursor without mixing tabs', () => {
  const traces = new Traces(),
    { trace_id } = traces.start(1, () => true);
  traces.add(2, 'console', { message: 'foreign' });
  traces.add(undefined, 'network', {});
  traces.add(1, 'console', { message: 'submit' });
  traces.add(1, 'network', { status: 400 });
  const first = traces.read(1, { trace_id, limit: 1 });
  assert.deepEqual(
    first.events.map((e) => e.kind),
    ['console'],
  );
  assert.equal(first.has_more, true);
  assert.deepEqual(
    traces.read(1, { trace_id, after: first.next_cursor }).events.map((e) => e.kind),
    ['network'],
  );
  assert.throws(() => traces.read(2, { trace_id }), /Unknown trace/);
});
it('stopping preserves unread pages and releases capacity only after the last page', () => {
  const traces = new Traces(),
    { trace_id } = traces.start(1, () => true);
  traces.add(1, 'console', {});
  traces.add(1, 'network', {});
  const first = traces.read(1, { trace_id, limit: 1 }, true);
  traces.add(1, 'console', { late: true });
  const last = traces.read(1, { trace_id, after: first.next_cursor });
  assert.notEqual(last.stopped_at, null);
  assert.equal(last.events.length, 1);
  assert.equal(last.has_more, false);
  assert.throws(() => traces.read(1, { trace_id }), /Unknown trace/);
});
it('revoked traces never collect later human or newly authorized activity', () => {
  const traces = new Traces();
  let allowed = true;
  const { trace_id } = traces.start(1, () => allowed);
  traces.add(1, 'console', { before: true });
  allowed = false;
  traces.add(1, 'console', { human: true });
  allowed = true;
  traces.add(1, 'console', { after: true });
  assert.equal(traces.read(1, { trace_id }).events.length, 1);
});
it('reports evicted records instead of implying a complete trace', () => {
  const traces = new Traces(),
    { trace_id } = traces.start(1, () => true);
  for (let i = 0; i <= TRACE_EVENTS_MAX; i++) traces.add(1, 'network', { i });
  const page = traces.read(1, { trace_id, limit: TRACE_EVENTS_MAX });
  assert.equal(page.events.length, TRACE_EVENTS_MAX);
  assert.equal(page.dropped, 1);
  assert.equal(page.events[0].sequence, 2);
  assert.equal(page.has_more, false);
});
it('rejects invalid pagination before stopping and releases destroyed-tab traces', () => {
  const traces = new Traces(),
    { trace_id } = traces.start(1, () => true);
  for (const query of [{ after: -1 }, { after: 1 }, { limit: 0 }, { limit: Infinity }])
    assert.throws(() => traces.read(1, { trace_id, ...query }, true), /Invalid trace/);
  assert.equal(traces.read(1, { trace_id }).stopped_at, null);
  traces.remove(1);
  assert.throws(() => traces.read(1, { trace_id }), /Unknown trace/);
});
it('caps simultaneous traces and rejects a revoked start', () => {
  const traces = new Traces();
  assert.throws(() => traces.start(1, () => false), /unavailable/);
  for (let i = 0; i < TRACE_MAX; i++) traces.start(i, () => true);
  assert.throws(() => traces.start(9, () => true), /Stop an existing/);
  traces.remove(0);
  assert.ok(traces.start(9, () => true).trace_id);
});
