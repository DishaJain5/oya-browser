/** Labels and empty states for separate, theme-aware collection tabs. */
import type { LibraryKind } from '../../../shared/library.ts';
/** Empty states distinguish no saved data from an overly narrow search. */
const EMPTY = {
  bookmarks: [
    'Keep something worth coming back to.',
    'Use the star in the address bar to save a page. It will appear here.',
  ],
  history: ['Your next visit starts here.', 'Pages you visit in this profile will appear here.'],
  downloads: [
    'No downloads yet.',
    'Files you download in this profile are saved to your Downloads folder and listed here.',
  ],
  search: ['No pages found', 'Try fewer words, a website name, or clear your filters.'],
};
/** Each collection's name. */
const NAMES: Record<LibraryKind, string> = { bookmarks: 'Bookmarks', history: 'History', downloads: 'Downloads' };
/** Each collection's line under its name. */
const DESCRIPTIONS: Record<LibraryKind, string> = {
  bookmarks: 'Your saved pages, ready when you need them.',
  history: 'The pages you visited, from latest to earliest.',
  downloads: 'Files you downloaded, saved to your Downloads folder.',
};
/** What one entry of each collection is called, for counts. */
const UNITS: Record<LibraryKind, [string, string]> = {
  bookmarks: ['page', 'pages'],
  history: ['page', 'pages'],
  downloads: ['file', 'files'],
};
/** Assign text only to a packaged element in this tab. */
const label = (root: HTMLElement, id: string, value: string): void => {
  root.querySelector(`#${id}`)!.textContent = value;
};
/** Headings stay concise and match the selected tab's name. */
export function heading(root: HTMLElement, kind: LibraryKind, total: number, count: number, site: string): void {
  const name = NAMES[kind];
  label(root, 'collection-name', name);
  label(root, 'collection-count', String(total));
  label(root, 'title', name);
  label(root, 'open-hint', kind === 'downloads' ? 'Files open in their app' : 'Pages open in a new tab');
  description(root, kind, count, site);
  searchLabels(root, kind);
}
/** The current collection is named for screen readers and in the search hint. */
function searchLabels(root: HTMLElement, kind: LibraryKind): void {
  const search = root.querySelector<HTMLInputElement>('#search')!;
  search.setAttribute('aria-label', `Search ${kind}`);
  search.placeholder = kind === 'downloads' ? 'Search downloads by file name or URL' : `Search ${kind} by title or URL`;
}
/** A blank collection is different from a search with no matches. */
export function emptyState(root: HTMLElement, kind: LibraryKind, filtered: boolean): void {
  const copy = filtered ? EMPTY.search : EMPTY[kind];
  label(root, 'empty-title', copy[0]);
  label(root, 'empty-detail', copy[1]);
  root.querySelector<HTMLElement>('#clear-search')!.hidden = !filtered;
}

/** The description and filtered count give enough context without a second title. */
function description(root: HTMLElement, kind: LibraryKind, count: number, site: string): void {
  label(root, 'description', DESCRIPTIONS[kind]);
  const suffix = site ? ` on ${site}` : '';
  label(root, 'result-count', `${count.toLocaleString()} ${UNITS[kind][count === 1 ? 0 : 1]}${suffix}`);
}
