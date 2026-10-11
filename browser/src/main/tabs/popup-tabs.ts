/**
 * Windows the page opened, kept on the tab list.
 *
 * A sign-on handoff, a payer passing a case to a delegated vendor, calls
 * `window.open` and then keeps using what it got back, so those windows have to
 * stay real windows rather than become tabs. That left them somewhere an agent
 * could neither see nor drive, so they are adopted here: listed, switched to and
 * closed like any tab, while remaining their own window on screen.
 */
import type { BrowserWindow } from 'electron';
import type { Tab, TabStrip } from './types.ts';

/** The tab manager, as far as adopting a window goes. */
export interface WindowHost extends TabStrip {
  /** The id the next tab gets. */
  nextTabId: number;
  /** Follows a tab's title. */
  titleChanged(tab: Tab, title: string): void;
  /** Shows a tab, mounting its page on the shell. */
  activateTab?(id: number): void;
}

/** The tab each popup was opened over, which the person returns to when it closes. */
const openedOver = new WeakMap<BrowserWindow, number | null>();

/** A window the page opened, as the tab list sees it: only the webContents a command needs. */
export function windowTab(id: number, win: BrowserWindow): Tab {
  return {
    id,
    window: win,
    view: { webContents: win.webContents },
    title: win.webContents.getTitle?.() || 'New Tab',
    url: win.webContents.getURL?.() || '',
  };
}

/** Puts `win` on the manager's list and returns its tab id, or null if it is already there. */
export function adoptWindow(manager: WindowHost, win: BrowserWindow | null | undefined, id?: number): number | null {
  if (!win?.webContents || manager.list.some((t) => t.window === win)) return null;
  const tab = windowTab(id ?? manager.nextTabId++, win);
  manager.list.push(tab);
  win.webContents.on('page-title-updated', (_e, title) => manager.titleChanged(tab, title));
  win.on('closed', () => forgetWindow(manager, win));
  openedOver.set(win, manager.activeTabId);
  manager.activeTabId = tab.id;
  return (manager.sendTabList(), tab.id);
}

/** Drops a popup's tab when the window itself goes; if it was active, the tab it opened over shows again, not the first tab. */
export function forgetWindow(manager: WindowHost, win: BrowserWindow): void {
  const idx = manager.list.findIndex((t) => t.window === win);
  if (idx === -1) return;
  const [popup] = manager.list.splice(idx, 1);
  if (popup.id !== manager.activeTabId) return manager.sendTabList();
  manager.activeTabId = null;
  returnTo(manager, manager.list.find((t) => t.id === openedOver.get(win)) ?? manager.list[0]);
}

/** Shows `back` again after a popup closed: mounted through the manager when it can, else just marked active. */
function returnTo(manager: WindowHost, back: Tab | undefined): void {
  if (back && manager.activateTab) return manager.activateTab(back.id);
  manager.activeTabId = back?.id ?? null;
  manager.sendTabList();
}

/** Brings a popup's own window to the front; there is nothing to mount on the shell. */
export function raiseWindow(manager: TabStrip, tab: Tab): void {
  try {
    tab.window?.focus();
  } catch {}
  manager.sendTabList();
}

/** Closes a popup's window. Its own 'closed' handler has already taken it off the list. */
export function closeWindowTab(tab: Tab): void {
  try {
    if (!tab.window?.isDestroyed()) tab.window?.close();
  } catch {}
}
