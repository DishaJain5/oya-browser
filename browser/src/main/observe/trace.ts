/** Bounded tab-owned network and console traces; no debugger, headers, or response bodies. */
import { randomUUID } from 'node:crypto';
import { TRACE_MAX, TRACE_EVENTS_MAX, DEFAULT_READ_LIMIT } from './constants.ts';
/** A sanitized native observation with its chronological cursor. */
interface TraceEvent {
  /** Monotonic sequence, including events evicted from the ring. */
  sequence: number;
  /** Console or native network lifecycle event. */
  kind: string;
  /** Sanitized observation produced by the browser. */
  data: object;
}
/** A trace cannot migrate to another tab, persona, or control revision. */
interface Trace {
  /** Native page identity. */
  target: number;
  /** Rechecked on capture and reads. */
  allowed(): boolean;
  /** Creation time. */
  started_at: number;
  /** Stop time, including a revoked ownership grant. */
  stopped_at: number | null;
  /** Last assigned event cursor. */
  sequence: number;
  /** Bounded chronological records. */
  events: TraceEvent[];
}
/** Trace reads are paginated without silently skipping retained events. */
export interface TraceQuery {
  /** Opaque id returned by start_trace. */
  trace_id?: string;
  /** Only events after this sequence. */
  after?: number;
  /** Maximum events returned. */
  limit?: number;
}
/** One browser's bounded trace registry; all access still passes command admission. */
export class Traces {
  /** Opaque identities never refer to another browser's trace. */
  private readonly traces = new Map<string, Trace>();
  /** Start without clearing existing console/network history. */
  start(target: number, allowed: () => boolean) {
    if (!allowed()) throw Error('Trace target is unavailable or control changed');
    if (this.traces.size >= TRACE_MAX) throw Error('Stop an existing trace before starting another');
    const trace_id = randomUUID(),
      started_at = Date.now();
    this.traces.set(trace_id, { target, allowed, started_at, stopped_at: null, sequence: 0, events: [] });
    return { trace_id, started_at, target, capacity: TRACE_EVENTS_MAX };
  }
  /** Capture only matching exact-page observations while the original grant is current. */
  add(target: number | undefined, kind: string, data: object): void {
    for (const trace of this.traces.values()) {
      if (trace.stopped_at !== null || trace.target !== target) continue;
      if (!captureAllowed(trace)) continue;
      trace.events.push({ sequence: ++trace.sequence, kind, data });
      if (trace.events.length > TRACE_EVENTS_MAX) trace.events.shift();
    }
  }
  /** Destroyed tabs cannot strand trace capacity or retain callbacks into retired personas. */
  remove(target: number): void {
    for (const [id, trace] of this.traces) if (trace.target === target) this.traces.delete(id);
  }
  /** Read a stable chronological slice; stopping removes the trace after its final read. */
  read(target: number, query: TraceQuery, stop = false) {
    const trace = this.traces.get(query.trace_id!);
    if (!trace || trace.target !== target) throw Error('Unknown trace for this tab');
    const result = tracePage(trace, query);
    if (stop) trace.stopped_at = result.stopped_at ??= Date.now();
    if (trace.stopped_at !== null && !result.has_more) this.traces.delete(query.trace_id!);
    return result;
  }
}
/** Validate cursors before changing a trace or allocating a response. */
function tracePage(trace: Trace, { after = 0, limit = DEFAULT_READ_LIMIT }: TraceQuery) {
  if (!Number.isSafeInteger(after) || after < 0 || after > trace.sequence) throw Error('Invalid trace cursor');
  if (!Number.isSafeInteger(limit) || limit < 1 || limit > TRACE_EVENTS_MAX) throw Error('Invalid trace limit');
  if (!trace.allowed()) trace.stopped_at ??= Date.now();
  const events = trace.events.filter((entry) => entry.sequence > after).slice(0, limit);
  const next_cursor = events.at(-1)?.sequence ?? after;
  return pageResult(trace, events, next_cursor, after);
}

/** Revoked capture never resumes, even when subsequent events belong to a new control grant. */
function captureAllowed(trace: Trace): boolean {
  if (trace.allowed()) return true;
  trace.stopped_at = Date.now();
  return false;
}
/** Explicit cursor and loss metadata distinguish an incomplete trace from no activity. */
function pageResult(trace: Trace, events: TraceEvent[], next_cursor: number, after: number) {
  return {
    started_at: trace.started_at,
    stopped_at: trace.stopped_at,
    events,
    next_cursor,
    has_more: next_cursor < trace.sequence,
    dropped: Math.max(0, trace.sequence - trace.events.length - after),
  };
}
