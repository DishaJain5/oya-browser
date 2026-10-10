/** Passive native request lifecycle tracing, isolated from governance's blocking request hook. */
import { randomUUID } from 'node:crypto';
import type { OnCompletedListenerDetails } from 'electron';
import { safeUrl, type Observer } from './observer.ts';
import { NETWORK_MAX } from './constants.ts';
/** Native lifecycle events share these fields; phases may supply additional response metadata. */
type Details = Pick<
  OnCompletedListenerDetails,
  'id' | 'url' | 'method' | 'resourceType' | 'webContentsId' | 'timestamp'
> & {
  /** Real response status when supplied by the engine. */ statusCode?: number;
  /** Native transport failure. */ error?: string;
  /** Redirect destination, sanitized before storage. */ redirectURL?: string;
};
/** Request identity is session-local, with a public random id retained across redirects. */
interface Request {
  /** Opaque public correlation key. */ id: string;
  /** Native request-header timestamp; absent when capture began after the request. */ started: number | null;
}
/** One persistent persona session's bounded request correlations. */
export class NetworkTrace {
  /** Native ids cannot collide with a different session's map. */
  private readonly requests = new Map<number, Request>();
  /** Sink owns the bounded trace rings. */
  private readonly observer: Observer;
  /** Capture no headers, bodies or cookie data. */
  constructor(observer: Observer) {
    this.observer = observer;
  }
  /** Record only native HTTP traffic with exact source identity. */
  event(phase: string, details: Details): void {
    if (!/^https?:\/\//i.test(details.url)) return;
    const request = this.request(phase, details);
    this.observer.traces.add(details.webContentsId, 'network', networkMetadata(phase, details, request));
    if (phase === 'complete' || phase === 'error') this.requests.delete(details.id);
  }
  /** Redirects retain identity; overflow evicts only correlation, never blocks browsing. */
  private request(phase: string, details: Details): Request {
    const previous = this.requests.get(details.id);
    if (previous) return previous;
    if (this.requests.size >= NETWORK_MAX) this.requests.delete(this.requests.keys().next().value!);
    const request = { id: randomUUID(), started: phase === 'request' ? details.timestamp : null };
    this.requests.set(details.id, request);
    return request;
  }
}
/** Preserve native timing honestly: elapsed since headers, not fabricated DNS/TLS timings. */
function networkMetadata(phase: string, d: Details, request: Request) {
  return {
    at: d.timestamp,
    phase,
    url: safeUrl(d.url),
    method: d.method,
    type: d.resourceType,
    ...responseMetadata(d, request),
  };
}

/** Response-only fields are unknown until the native phase supplies them. */
function responseMetadata(d: Details, request: Request) {
  return {
    request_id: request.id,
    status: d.statusCode ?? null,
    error: d.error && d.error !== 'net::OK' ? d.error : null,
    redirect_url: d.redirectURL ? safeUrl(d.redirectURL) : null,
    started_at: request.started,
    elapsed_ms: request.started === null ? null : Math.max(0, d.timestamp - request.started),
  };
}
