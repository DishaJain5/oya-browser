/** The person's downloads: where the list is kept, how much of it, and how often progress is told. */

/** The list's file in the user-data folder. */
export const DOWNLOADS_FILE = 'downloads.json';
/** The newest downloads kept in the list; older ones drop off (the files stay on disk). */
export const DOWNLOADS_KEPT = 200;
/** At most one save and one shell update per this many milliseconds while bytes arrive. */
export const PROGRESS_INTERVAL_MS = 500;
/** The highest " (n)" tried for a free file name before giving up. */
export const NAME_ATTEMPTS = 1000;
