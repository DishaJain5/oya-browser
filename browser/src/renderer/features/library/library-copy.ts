/** Labels and empty states for separate, theme-aware collection tabs. */
import type { LibraryKind } from '../../../shared/library.ts';
/** Empty states distinguish no saved data from an overly narrow search. */
const EMPTY = {
  bookmarks: [
    'Keep something worth coming back to.',
    'Use the star in the address bar to save a page. It will appear here.',
  ],
  history: ['Your next visit starts here.', 'Pages you visit in this profile will appear here.'],
  search: ['No pages found', 'Try fewer words, a website name, or clear your filters.'],
};
/** Assign text only to a packaged element in this tab. */
const label = (root: HTMLElement, id: string, value: string): void => {
  root.querySelector(`#${id}`)!.textContent = value;
};
/** Headings stay concise and match the selected tab's name. */
export function heading(root: HTMLElement, kind: LibraryKind, total: number, count: number, site: string): void {
  const name = kind === 'bookmarks' ? 'Bookmarks' : 'History';
  label(root, 'collection-name', name);
  label(root, 'collection-count', String(total));
  label(root, 'title', name);
  description(root, kind, count, site);
  searchLabels(root, kind);
}
/** The current collection is named for screen readers and in the search hint. */
function searchLabels(root: HTMLElement, kind: LibraryKind): void {
  const search = root.querySelector<HTMLInputElement>('#search')!;
  search.setAttribute('aria-label', `Search ${kind}`);
  search.placeholder = `Search ${kind} by title or URL`;
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
  const copy =
    kind === 'bookmarks'
      ? 'Your saved pages, ready when you need them.'
      : 'The pages you visited, from latest to earliest.';
  label(root, 'description', copy);
  const suffix = site ? ` on ${site}` : '';
  label(root, 'result-count', `${count.toLocaleString()} ${count === 1 ? 'page' : 'pages'}${suffix}`);
}
