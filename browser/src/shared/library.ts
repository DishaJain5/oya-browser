/** Private library presentation and commands; no website receives this bridge. */
export const LIBRARY = {
  PAGE_SIZE: 50,
} as const;
/** One local saved page. */
export interface SavedPage {
  /** Address checked by the main-process library. */
  url: string;
  /** Literal page title. */
  title: string;
  /** Latest visit or save time. */
  time: number;
}
/** Current profile only. */
export interface LibrarySnapshot {
  /** Explicitly saved pages. */
  bookmarks: SavedPage[];
  /** Latest visit per address. */
  history: SavedPage[];
}
/** Two independent collections share the search interface. */
export type LibraryKind = keyof LibrarySnapshot;
/** Commands are validated again against current main-process data. */
export type LibraryAction = 'open' | 'bookmark' | 'remove' | 'manage';

/** Each tab opens exactly one collection, with no combined library navigation. */
export interface LibraryPresentation extends LibrarySnapshot {
  /** Fixed for the lifetime of this tab. */
  kind: LibraryKind;
}
