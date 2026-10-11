/**
 * The one way a tab loads a page: after its protection has settled, and never
 * in a tab that could not be protected. The first page, the address bar and a
 * navigate command all come through here, so none of them can load a page
 * with no fingerprint while setup is still pending or after it failed.
 */
import type { LoadURLOptions } from 'electron';
import { TAB_UNPROTECTED, TAB_UNPROTECTED_DESKTOP } from './constants.ts';
import type { Tab, TabListSink } from './types.ts';

/** The code a caller branches on, carried on the error to the server. */
const UNPROTECTED_CODE = 'tab_unprotected';

/** A tab as far as loading in it goes: its view, and its protection while it settles. */
export type LoadableTab = Pick<Tab, 'view' | 'setup' | 'protection'>;

/** Any tab, as far as its protection goes: an adopted popup or a front-door tab may have no setup at all. */
type ProtectableTab = object & Partial<Pick<Tab, 'setup' | 'protection'>>;

/** An error that carries a code a caller branches on. */
interface CodedError extends Error {
  /** What kind of failure it is. */
  code?: string;
}

/** The refusal for a tab that could not be protected. */
export function unprotected(): CodedError {
  const err: CodedError = new Error(TAB_UNPROTECTED);
  err.code = UNPROTECTED_CODE;
  return err;
}

/** Whether `err` is that refusal. */
export const isUnprotected = (err: unknown): boolean =>
  typeof err === 'object' && err !== null && 'code' in err && err.code === UNPROTECTED_CODE;

/**
 * Resolves once the tab's protection has settled; throws for a tab it failed
 * on. `tab.setup` always settles (two bounded attempts), and a tab the app
 * adopted from a popup has none, so it goes on as it always has.
 */
export async function whenProtected(tab: ProtectableTab | null | undefined): Promise<void> {
  await tab?.setup;
  if (tab?.protection === 'failed') throw unprotected();
}

/** Loads `url` once the tab's protection has settled; refuses a tab it failed on. */
export async function loadInTab(tab: LoadableTab, url: string, options?: LoadURLOptions): Promise<void> {
  await whenProtected(tab);
  return tab.view.webContents.loadURL(url, options);
}

/** Shows the person why the tab stays empty, and offers no Reload. */
export function showUnprotected(tabs: TabListSink, tab: Tab): void {
  tab.navigationPending = false;
  tab.loadError = TAB_UNPROTECTED_DESKTOP;
  tabs.sendTabList();
}

/**
 * An agent's navigate: done once the new document is parsed, like a person seeing the
 * page, not when its last image or tracker finishes. A request that never ends held the
 * command for its whole 90s timeout. A failed main-frame load still rejects.
 */
export async function loadUntilReady(tab: LoadableTab, url: string): Promise<void> {
  const ready = documentReady(tab.view.webContents);
  const load = loadInTab(tab, url).finally(ready.cancel);
  load.catch(() => {});
  await Promise.race([load, ready.promise]);
}

/** The webContents a tab loads in. */
type Contents = LoadableTab['view']['webContents'];

/** Chromium's ERR_ABORTED: the previous, still-loading document giving way to this navigation. */
const ABORTED = -3;

/** What one agent navigation has seen so far. */
interface LoadState {
  /** The new document committed, so its dom-ready is this navigation's. */
  committed: boolean;
  /** The main frame failed: the load's own rejection reports it. */
  failed: boolean;
}

/** Resolves on the new document's dom-ready, unless its main frame failed first; `cancel` drops the listeners. */
function documentReady(contents: Contents) {
  const state: LoadState = { committed: false, failed: false };
  let resolve = (): void => {};
  const promise = new Promise<void>((done) => (resolve = done));
  const events = readyEvents(state, () => state.committed && !state.failed && resolve());
  for (const [name, listener] of events) contents.on(name as 'dom-ready', listener);
  const cancel = () => events.forEach(([name, listener]) => contents.off(name as 'dom-ready', listener));
  return { promise, cancel };
}

/** Commit, failure and readiness listeners: a dom-ready counts only after this navigation committed. */
function readyEvents(state: LoadState, ready: () => void) {
  const commit = () => void (state.committed = true);
  const fail = (_e: unknown, code: number, _text: string, _url: string, main: boolean) =>
    void (state.failed ||= main && code !== ABORTED);
  return [
    ['did-navigate', commit],
    ['did-fail-load', fail],
    ['dom-ready', ready],
  ] as [string, (...a: never[]) => void][];
}
