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
/** Where a download stands: still coming, saved, stopped by the person, or failed. */
export type DownloadState = 'progressing' | 'completed' | 'cancelled' | 'interrupted';
/** One file the person downloaded: its name as the title, the address it came from, when it started. */
export interface DownloadEntry extends SavedPage {
  /** Names it to the main process, which alone knows where it was saved. */
  id: string;
  /** Where it stands. */
  state: DownloadState;
  /** Bytes saved so far. */
  received: number;
  /** Its size, or 0 when the site did not say. */
  total: number;
}
/** Current profile only. */
export interface LibrarySnapshot {
  /** Explicitly saved pages. */
  bookmarks: SavedPage[];
  /** Latest visit per address. */
  history: SavedPage[];
  /** Files downloaded in this profile, newest first. */
  downloads: DownloadEntry[];
}
/** Three independent collections share the search interface. */
export type LibraryKind = keyof LibrarySnapshot;
/** Commands are validated again against current main-process data; 'reveal' shows a download in its folder. */
export type LibraryAction = 'open' | 'bookmark' | 'remove' | 'manage' | 'reveal';

/** Each tab opens exactly one collection, with no combined library navigation. */
export interface LibraryPresentation extends LibrarySnapshot {
  /** Fixed for the lifetime of this tab. */
  kind: LibraryKind;
}
