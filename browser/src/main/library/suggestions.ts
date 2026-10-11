/**
 * Private, bounded address completion from the active persona's own library.
 * Ranked as people expect from a browser's address bar: a site whose name starts
 * with what was typed comes first, its home page before its deep links (and offered
 * even when only deep pages were visited); then addresses that start with it; then
 * a word in a title; then anything containing it. Recency breaks ties, a bookmark
 * edges out history, and no one site fills the list.
 */
import type { AddressSuggestion } from '../../shared/ipc.ts';
import type { BrowsingLibrary, LibraryEntry } from './library.ts';
import {
  ADDRESS_SUGGESTION_LIMIT,
  BOOKMARK_TITLE_LENGTH,
  QUERY_MAX_LENGTH,
  SUGGESTION_SCORES as SCORE,
  SUGGESTIONS_PER_SITE,
} from './constants.ts';

/** An address without its scheme and `www.`, as people type it. */
export const bare = (url: string): string => url.toLowerCase().replace(/^https?:\/\/(www\.)?/, '');

/** The host of a bare address. */
const hostOf = (url: string): string => bare(url).split(/[/?#]/)[0];

/** Whether the address is a site's home page. */
const isHome = (url: string): boolean => /^[^/?#]+\/?$/.test(bare(url));

/** A title is text, not HTML, and need not expose an unbounded page-provided string. */
function suggestion(entry: LibraryEntry, source: AddressSuggestion['source']): AddressSuggestion {
  return { url: entry.url, title: entry.title.slice(0, BOOKMARK_TITLE_LENGTH), source };
}

/** How well `item` answers `query`; 0 when it does not. */
function score(item: AddressSuggestion, query: string): number {
  const url = bare(item.url);
  const title = item.title.toLowerCase();
  const site = hostOf(item.url).startsWith(query) ? SCORE.site + (isHome(item.url) ? SCORE.home : 0) : 0;
  const match = site || (url.startsWith(query) ? SCORE.address : 0) || (wordStart(title, query) ? SCORE.word : 0);
  const any = match || (`${title} ${url}`.includes(query) ? SCORE.contains : 0);
  return any && any + (item.source === 'bookmark' ? SCORE.bookmark : 0);
}

/** Whether a word of `text` starts with `query`. */
const wordStart = (text: string, query: string): boolean =>
  text.split(/[^\p{L}\p{N}]+/u).some((w) => w.startsWith(query));

/** Home pages for matching sites the library holds only deep pages of, ranked as the sites. */
function siteHomes(items: AddressSuggestion[], query: string): AddressSuggestion[] {
  const homes = new Map<string, AddressSuggestion>();
  for (const item of items) {
    const host = hostOf(item.url);
    if (!host.startsWith(query) || homes.has(host)) continue;
    homes.set(host, { url: `${new URL(item.url).origin}/`, title: host, source: 'history' });
  }
  return [...homes.values()].filter((home) => !items.some((item) => bare(item.url) === bare(home.url)));
}

/** At most SUGGESTIONS_PER_SITE rows from any one site, in order. */
function spread(items: AddressSuggestion[]): AddressSuggestion[] {
  const perSite = new Map<string, number>();
  return items.filter((item) => {
    const seen = perSite.get(hostOf(item.url)) ?? 0;
    perSite.set(hostOf(item.url), seen + 1);
    return seen < SUGGESTIONS_PER_SITE;
  });
}

/** Only local HTTP(S) entries are returned, bookmarks winning duplicate URLs. */
export function addressSuggestions(library: BrowsingLibrary, value: unknown): AddressSuggestion[] {
  if (typeof value !== 'string' || value.length > QUERY_MAX_LENGTH) return [];
  const query = bare(value.trim());
  return query ? rank(libraryEntries(library), query) : [];
}

/** The best answers to `query`, best first, ties in the library's (recent) order. */
function rank(entries: AddressSuggestion[], query: string): AddressSuggestion[] {
  const scored = [...siteHomes(entries, query), ...entries].map((item, order) => ({
    item,
    order,
    s: score(item, query),
  }));
  const ranked = scored.filter((x) => x.s > 0).sort((a, b) => b.s - a.s || a.order - b.order);
  return spread(ranked.map((x) => x.item)).slice(0, ADDRESS_SUGGESTION_LIMIT);
}

/** Preserve bookmark preference and recency while returning each address only once. */
function deduplicate(entries: AddressSuggestion[]): AddressSuggestion[] {
  const seen = new Map<string, AddressSuggestion>();
  for (const entry of entries) if (!seen.has(entry.url)) seen.set(entry.url, entry);
  return [...seen.values()];
}

/** Bookmarks win duplicate addresses without displacing newer history-only entries. */
function libraryEntries(library: BrowsingLibrary): AddressSuggestion[] {
  const { bookmarks, history } = library.snapshot();
  return deduplicate([
    ...bookmarks.map((e) => suggestion(e, 'bookmark')),
    ...history.map((e) => suggestion(e, 'history')),
  ]);
}
