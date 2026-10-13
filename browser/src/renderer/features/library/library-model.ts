/** Literal search, site filters and calendar groups for the private library view. */
import type { SavedPage } from '../../../shared/library.ts';
/** Select controls use fixed sort choices. */
export type LibrarySort = 'recent' | 'title' | 'site';
/** Every whitespace-separated word must occur somewhere in the page title or address. */
export function matching(pages: SavedPage[], query: string, site: string, sort: LibrarySort): SavedPage[] {
  const terms = query.toLocaleLowerCase().trim().split(/\s+/).filter(Boolean);
  const found = pages.filter(
    (page) =>
      (!site || host(page) === site) &&
      terms.every((term) => `${page.title} ${page.url}`.toLocaleLowerCase().includes(term)),
  );
  return found.sort((a, b) => compare(a, b, sort));
}
/** Full host disambiguates similarly named saved pages, without tracking favicon requests. */
export function host(page: SavedPage): string {
  return new URL(page.url).host;
}
/** Local calendar boundaries keep yesterday correct across daylight-saving changes. */
export function dayLabel(time: number, now = new Date()): string {
  const date = new Date(time);
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  if (date.toDateString() === now.toDateString()) return 'Today';
  if (date.toDateString() === yesterday.toDateString()) return 'Yesterday';
  return date.toLocaleDateString(undefined, { weekday: 'long', month: 'short', day: 'numeric', year: 'numeric' });
}
/** Most-used sites come first, with stable alphabetical ties. */
export function sites(pages: SavedPage[]): [string, number][] {
  const counts = new Map<string, number>();
  for (const page of pages) counts.set(host(page), (counts.get(host(page)) ?? 0) + 1);
  return [...counts].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
}

/** Compare without mutating the source snapshot's order. */
function compare(a: SavedPage, b: SavedPage, sort: LibrarySort): number {
  if (sort === 'title') return a.title.localeCompare(b.title);
  if (sort === 'site') return host(a).localeCompare(host(b)) || b.time - a.time;
  return b.time - a.time;
}
