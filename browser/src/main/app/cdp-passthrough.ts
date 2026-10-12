/**
 * Page-session passthrough for the external CDP front door: standard page
 * methods the native layer cannot answer in a protocol-consistent way (frame,
 * context, node and loader ids that Playwright and Puppeteer correlate) go to
 * the exact tab's own debugger. Browser-wide state (targets, contexts,
 * cookies, downloads) stays native. Every call keeps the front door's target
 * ownership and human-control admission, refuses persona and browser-wide
 * methods, and opens only web addresses the egress rules allow.
 */
import type { Debugger, WebContents } from 'electron';
import { isWebAddress, NOT_A_WEB_ADDRESS } from '../tabs/navigation.ts';
import {
  PASSTHROUGH_CDP_VERSION,
  PASSTHROUGH_DOMAINS,
  PASSTHROUGH_REFUSED,
  PASSTHROUGH_TARGET_METHODS,
} from './constants.ts';

/** Delivers one passed-through event: method, params and the child CDP session it came from ('' for the page). */
export type PassthroughSink = (method: string, params: Record<string, unknown>, child: string) => void;

/** What the passthrough needs from the app: the exact tab view and the egress rule. */
export interface PassthroughDeps {
  /** The view of an authorized target; throws when it is not one. */
  view(target: string): { /** The tab's page. */ webContents: WebContents };
  /** Whether the egress rules allow opening this address. */
  allowedURL(url: string): boolean;
}

/**
 * Whether the door serves unmodified CDP clients: page passthrough and the browser's own profile.
 * OYA_CDP_PASSTHROUGH=0 restores the strict native door.
 */
export function clientCompatibility(): boolean {
  return process.env.OYA_CDP_PASSTHROUGH !== '0';
}

/** Whether `method` may pass through to a page's debugger at all; never when OYA_CDP_PASSTHROUGH=0. */
export function passesThrough(method: string): boolean {
  if (!clientCompatibility()) return false;
  const domain = method.split('.')[0];
  if (Object.hasOwn(PASSTHROUGH_REFUSED, method)) return true; // answered with its refusal below
  return PASSTHROUGH_DOMAINS.includes(domain) || PASSTHROUGH_TARGET_METHODS.includes(method);
}

/** The refusal for a method or its params, or '' to let it through. */
function refusal(method: string, params: Record<string, unknown>, allowedURL: (url: string) => boolean): string {
  if (Object.hasOwn(PASSTHROUGH_REFUSED, method)) return PASSTHROUGH_REFUSED[method];
  const opens = method === 'Page.navigate' || (method === 'Fetch.continueRequest' && params.url !== undefined);
  if (!opens) return '';
  if (!isWebAddress(params.url)) return NOT_A_WEB_ADDRESS;
  return allowedURL(String(params.url)) ? '' : 'Blocked by this browser’s egress rules';
}

/** Replaces every quoted occurrence of `from` with `to` in a JSON-shaped value. */
function swapId<T>(value: T, from: string, to: string): T {
  if (!from || from === to || value === undefined) return value;
  return JSON.parse(JSON.stringify(value).split(`"${from}"`).join(`"${to}"`)) as T;
}

/** One connection's passthrough: id translation per tab, and debugger listeners it owns. */
export class CdpPassthrough {
  /** App lookups. */
  private readonly deps: PassthroughDeps;
  /** Each tab's CDP target id (its main frame id), keyed by the front door's target id. */
  private readonly cdpIds = new Map<string, string>();
  /** Debugger listeners this connection added, for cleanup. */
  private readonly listeners = new Set<() => void>();

  /** Keeps the app lookups. */
  constructor(deps: PassthroughDeps) {
    this.deps = deps;
  }

  /** Sends one command to the exact tab's debugger, translating the main frame id both ways. */
  async send(target: string, method: string, params: Record<string, unknown>, child = ''): Promise<unknown> {
    const why = refusal(method, params, this.deps.allowedURL);
    if (why) throw Error(why);
    const dbg = this.debugger(target);
    const cdpId = await this.cdpId(target, dbg);
    const result = await dbg.sendCommand(method, swapId(params, target, cdpId), child || undefined);
    return swapId(result, cdpId, target);
  }

  /** Forwards the tab's debugger events to `emit` until the returned stop is called. */
  watch(target: string, emit: PassthroughSink): () => void {
    const dbg = this.debugger(target);
    void this.cdpId(target, dbg).catch(() => {});
    const listener = (_e: unknown, method: string, params: Record<string, unknown>, child?: string): void =>
      emit(method, swapId(params, this.cdpIds.get(target) ?? '', target), child ?? '');
    dbg.on('message', listener);
    const stop = (): void => void dbg.off('message', listener);
    this.listeners.add(stop);
    return () => (stop(), this.listeners.delete(stop));
  }

  /** Removes every debugger listener this connection added. */
  dispose(): void {
    for (const stop of this.listeners) stop();
    this.listeners.clear();
  }

  /** The exact tab's debugger, attached. */
  private debugger(target: string): Debugger {
    const contents = this.deps.view(target).webContents;
    if (contents.isDestroyed()) throw Error('Target is closed');
    if (!contents.debugger.isAttached()) contents.debugger.attach(PASSTHROUGH_CDP_VERSION);
    return contents.debugger;
  }

  /** The tab's CDP target id, read once from its own debugger. */
  private async cdpId(target: string, dbg: Debugger): Promise<string> {
    if (!this.cdpIds.has(target)) {
      const { targetInfo } = (await dbg.sendCommand('Target.getTargetInfo')) as TargetInfoReply;
      this.cdpIds.set(target, targetInfo.targetId);
    }
    return this.cdpIds.get(target)!;
  }
}

/** The part of Target.getTargetInfo the id translation reads. */
interface TargetInfoReply {
  /** The tab's own target. */
  targetInfo: {
    /** Its CDP id, which is also its main frame's id. */
    targetId: string;
  };
}
