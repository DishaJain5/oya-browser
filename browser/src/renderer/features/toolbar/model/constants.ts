/**
 * The navigation toolbar's words: button names, the address bar's status,
 * the window title, and the Ask button's titles.
 */

/** What the toolbar says. */
export const TEXT = {
  /** The toolbar's name. */
  toolbar: 'Browser navigation',
  library: 'History',
  /** The bookmarks bar's name. */
  bookmarks: 'Bookmarks',
  /** What the bar says before any bookmark. */
  bookmarksEmpty: 'Bookmark a page to keep it here.',
  /** The bar's way to every bookmark and the history. */
  allBookmarks: 'Bookmarks',
  /** The star's name when the page is not saved. */
  bookmarkPage: 'Bookmark this page',
  /** The star's name when it is. */
  unbookmarkPage: 'Remove bookmark',
  /** The bar's hide button. */
  hideBookmarks: 'Hide bookmarks bar',
  libraryTitle: 'History (⌘/Ctrl Y)',
  back: 'Back',
  forward: 'Forward',
  reload: 'Reload page',
  stop: 'Stop loading',
  /** The address bar's name and placeholder. */
  address: 'Search or enter URL',
  /** The address bar's status while the page loads, and after it failed. */
  loading: 'Loading…',
  failed: 'Load failed',
  /** The window's title, alone and after a page's title. */
  app: 'Oya Browser',
  titleJoin: ', ',
  /** The Ask button's label and titles. */
  ask: 'Assistant',
  askTitle: 'Open Assistant (⌘/Ctrl Shift D)',
  askRecordingTitle: 'Oya Agent · recording',
} as const;

/** The key that sends the address bar. */
export const SUBMIT_KEY = 'Enter';

/** A dedicated overlay prevents native page views from covering the suggestions. */
export const ADDRESS_OVERLAY = 'address';
