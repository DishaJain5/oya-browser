/**
 * Page sessions whose standard methods pass through to the tab (the backend's
 * optional `passthrough`), with their event streams and the iframe and worker
 * sessions the tab attaches itself. Oya.* methods and every browser-scoped
 * method stay native. Ownership is the protocol's: a page session must map to
 * an authorized target, and a child session only exists under one.
 */
import type { NativeBackend, NativeCommand } from './types.ts';

/** Delivers one event to the client, under `sessionId` when it has one. */
type Emit = (event: object) => void;

/** Where a passed-through session's commands go. */
interface Route {
  /** The front door's target id of the tab. */
  target: string;
  /** The tab's own child session ('' for the page itself). */
  child: string;
}

/** One connection's passed-through page and child sessions. */
export class PassthroughSessions {
  /** The app backend, which owns the actual passthrough. */
  private readonly backend: NativeBackend;
  /** Client event delivery. */
  private readonly emit: Emit;
  /** Event forwarding per page session key (its session id, or the pinned target). */
  private readonly watches = new Map<string, () => void>();
  /** Child sessions the tab attached (iframes, workers): child id -> its tab and parent key. */
  private readonly children = new Map<string, Route & { /** Parent page session key. */ parent: string }>();

  /** Keeps the backend and the client's event sink. */
  constructor(backend: NativeBackend, emit: Emit) {
    this.backend = backend;
    this.emit = emit;
  }

  /** Whether `command` passes through: a standard method the backend passes, on a page or child session. */
  handles(command: NativeCommand, pageSession: boolean): boolean {
    if (!this.backend.passthrough || command.method.startsWith('Oya.')) return false;
    const child = !!command.sessionId && this.children.has(command.sessionId);
    return (child || pageSession) && !!this.backend.passesThrough?.(command.method);
  }

  /** Sends `command` to its tab: a child session's to that child, a page session's to the page. */
  send(command: NativeCommand, target: () => string): Promise<unknown> {
    const child = command.sessionId ? this.children.get(command.sessionId) : undefined;
    const route = child ?? { target: target(), child: '' };
    if (!child) this.watch(command.sessionId ?? route.target, route.target, command.sessionId);
    return this.backend.passthrough!(route.target, command.method, command.params, route.child);
  }

  /** Starts forwarding the tab's events for one page session, once. */
  private watch(key: string, target: string, sessionId?: string): void {
    if (this.watches.has(key) || !this.backend.passthroughEvents) return;
    const stop = this.backend.passthroughEvents(target, (method, params, child) =>
      this.deliver(key, target, sessionId, { method, params, child }),
    );
    this.watches.set(key, stop);
  }

  /** Delivers one tab event to the page session or the child session it came from. */
  private deliver(key: string, target: string, sessionId: string | undefined, event: TabEvent): void {
    if (event.child && !this.children.has(event.child)) return; // another client's child session
    this.track(key, target, event);
    const session = event.child || sessionId;
    this.emit({ method: event.method, params: event.params, ...(session ? { sessionId: session } : {}) });
  }

  /** Records child sessions the tab attaches and forgets those it detaches. */
  private track(key: string, target: string, { method, params }: TabEvent): void {
    const child = typeof params.sessionId === 'string' ? params.sessionId : '';
    if (method === 'Target.attachedToTarget' && child) this.children.set(child, { target, child, parent: key });
    if (method === 'Target.detachedFromTarget' && child) this.children.delete(child);
  }

  /** Stops one page session's forwarding and forgets its children. */
  release(key: string): void {
    this.watches.get(key)?.();
    this.watches.delete(key);
    for (const [id, route] of this.children) if (route.parent === key) this.children.delete(id);
  }

  /** Stops everything this connection forwards. */
  dispose(): void {
    for (const key of [...this.watches.keys()]) this.release(key);
  }
}

/** One event from the tab, with the child session it came from ('' for the page). */
interface TabEvent {
  /** CDP event name. */
  method: string;
  /** CDP event params. */
  params: Record<string, unknown>;
  /** The tab's child session, or ''. */
  child: string;
}
