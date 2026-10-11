/**
 * The bookmarks bar under the toolbar: the profile's bookmarks a click away, shown
 * unless the person turns it off (View > Always Show Bookmarks Bar, Cmd/Ctrl+Shift+B,
 * or the bar's own Hide). Turning it on or off re-lays every window, since the page
 * sits below it.
 */
import type { AppServices } from '../app/services.ts';
import type { ConfigStore } from '../app/config-store.ts';
import { BOOKMARKS_BAR_HEIGHT, BOOKMARKS_BAR_ITEMS, CHROME_HEIGHT } from './constants.ts';

/** What the bar reads and moves. */
type Deps = Pick<AppServices, 'config' | 'library' | 'windows' | 'layout' | 'shell'>;

/** What the shell is told: whether the bar shows, and the bookmarks on it. */
export interface BookmarksBarState {
  /** Whether it shows. */
  shown: boolean;
  /** Every bookmarked address, so the toolbar's star knows whether the page is saved. */
  saved: string[];
  /** The newest bookmarks first, as many as fit a bar. */
  bookmarks: Array<{ /** The address. */ url: string; /** The title. */ title: string }>;
  /** Lets the shell read it as a payload. */
  [key: string]: unknown;
}

/** Whether the bar shows: on unless the person turned it off. */
export const bookmarksBarShown = (config: Pick<ConfigStore, 'values'>): boolean =>
  config.values.ui?.bookmarksBar !== false;

/** How tall the window's chrome is: the strip and toolbar, and the bar when it shows. */
export const chromeHeight = (config: Pick<ConfigStore, 'values'>): number =>
  CHROME_HEIGHT + (bookmarksBarShown(config) ? BOOKMARKS_BAR_HEIGHT : 0);

/** The bar as the shell draws it. */
export function bookmarksBar(deps: Pick<Deps, 'config' | 'library'>): BookmarksBarState {
  const all = deps.library.snapshot().bookmarks;
  const bookmarks = all.slice(0, BOOKMARKS_BAR_ITEMS).map(({ url, title }) => ({ url, title }));
  return { shown: bookmarksBarShown(deps.config), saved: all.map((entry) => entry.url), bookmarks };
}

/** Tells every window's shell what the bar shows (after a bookmark changed, or the bar was turned on or off). */
export function announceBookmarks(deps: Deps): void {
  const state = bookmarksBar(deps);
  if (deps.windows) deps.windows.each((scope) => scope.shell.send('bookmarks-bar', state));
  else deps.shell.send('bookmarks-bar', state);
}

/** Shows or hides the bar, remembers it, and re-lays every window under it. */
export function setBookmarksBar(deps: Deps, shown: boolean): void {
  const ui = { ...deps.config.values.ui, bookmarksBar: shown };
  deps.config.merge({ ui });
  deps.config.save();
  if (deps.windows) deps.windows.each((scope) => scope.layout.layoutActiveTab());
  else deps.layout.layoutActiveTab();
  announceBookmarks(deps);
}
