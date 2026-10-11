/** Limits for the local browsing library. */
export const HISTORY_LIMIT = 2000;
/** Entries per native submenu, keeping long histories navigable. */
export const MENU_PAGE_SIZE = 30;
/** Titles in native menus should not stretch beyond the window. */
export const MENU_TITLE_LENGTH = 80;

/** Explicit confirmation is the second button; cancellation remains the default. */
export const CONFIRM_RESPONSE = 1;
/** Native wording and safe default for the destructive history action. */
export const CLEAR_HISTORY_CONFIRMATION = {
  type: 'question' as const,
  message: 'Clear browsing history for this profile?',
  detail: 'Bookmarks and website logins will be kept. This cannot be undone.',
  buttons: ['Cancel', 'Clear history'],
  defaultId: 0,
  cancelId: 0,
};

/** Default and maximum number of local library entries returned to an agent. */
export const QUERY_DEFAULT_LIMIT = 25;
/** Hard maximum per agent response. */
export const QUERY_MAX_LIMIT = 100;
/** Bound model input and returned titles without truncating navigable URLs. */
export const QUERY_MAX_LENGTH = 500;
/** Maximum display title accepted from an agent or returned in search. */
export const BOOKMARK_TITLE_LENGTH = 500;

/** Keep the omnibox dropdown small enough for short windows. */
export const ADDRESS_SUGGESTION_LIMIT = 8;
/** How much each kind of address-bar match weighs; the order matters, not the numbers. */
export const SUGGESTION_SCORES = {
  /** The site's name starts with what was typed. */
  site: 1000,
  /** On top of that, it is the site's home page. */
  home: 500,
  /** The address starts with what was typed. */
  address: 800,
  /** A word of the title starts with it. */
  word: 300,
  /** The title or address contains it somewhere. */
  contains: 100,
  /** A bookmark edges out history of the same weight. */
  bookmark: 50,
} as const;
/** Rows one site may take in the address bar's list, so other sites still show. */
export const SUGGESTIONS_PER_SITE = 3;
