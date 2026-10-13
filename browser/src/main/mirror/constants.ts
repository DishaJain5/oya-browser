/**
 * Every number the mirror import runs on, by name: how long to wait for the
 * launched browser and its CDP replies, how much to read, and the files that
 * carry a login. Kept here so no step hides a magic number.
 */

/** Milliseconds to wait for the launched browser's debugging port to answer. */
export const LAUNCH_READY_TIMEOUT_MS = 20_000;
/** Milliseconds between polls of the debugging port while it comes up. */
export const LAUNCH_POLL_MS = 200;
/** Milliseconds to wait for one CDP command's reply before giving up. */
export const CDP_COMMAND_TIMEOUT_MS = 15_000;
/** Milliseconds to wait for the server to answer an import before telling the person it did not. */
export const MIRROR_ANSWER_TIMEOUT_MS = 30_000;
/** The token a headless Chrome puts in its user agent, stripped so the UA reads as real. */
export const HEADLESS_TOKEN = /HeadlessChrome/g;
/** Milliseconds per second: Firefox stores cookie expiry in milliseconds, the pool in seconds. */
export const MS_PER_SECOND = 1000;
/** Bytes the sqlite3 cookie dump may return (128 MiB); a full Firefox jar is over the 1 MiB default. */
export const SQLITE_MAX_BUFFER = 134_217_728;

/**
 * The login-bearing files copied out of a real profile. Cookies decrypt over
 * CDP; these carry the rest of a session (localStorage, IndexedDB, service
 * workers) and are copied as files, since that is how Chromium stores them.
 */
export const PROFILE_STORES = ['Local Storage', 'IndexedDB', 'Service Worker', 'WebStorage', 'File System'] as const;

/** The files the launched browser needs to open the profile, copied before launch. */
export const LAUNCH_INPUTS = ['Local State'] as const;
/** Per-profile files the launched browser needs; joined under the profile dir. Never `Login Data`: saved passwords play no part in reading cookies, so they are not copied anywhere. */
export const PROFILE_INPUTS = ['Cookies', 'Network', 'Preferences'] as const;
/** How many finished imports the app remembers, newest first, for the profile dialog. */
export const IMPORTS_KEPT = 5;

/** Bounded retries for Windows releasing profile file handles. */
export const CLEANUP_RETRIES = 5;
/** Delay between cleanup attempts. */
export const CLEANUP_RETRY_MS = 200;
/** Time allowed for a capture process to exit before leaving its files intact. */
export const EXIT_TIMEOUT_MS = 5000;
/** Largest password export read: tens of thousands of logins, never an unrelated big file. */
export const MAX_PASSWORD_CSV_BYTES = 20_971_520; // 20 MiB
/** Milliseconds between 1601-01-01, where Chromium counts its times from, and the Unix epoch. */
export const WEBKIT_EPOCH_OFFSET_MS = 11_644_473_600_000;
/** Microseconds per millisecond: Chromium and Firefox keep visit and bookmark times in microseconds. */
export const US_PER_MS = 1000;
/** Most visits read from another browser's history, newest first: as many as Oya's own history keeps. */
export const IMPORTED_HISTORY_LIMIT = 2000;
/** The salt Chromium on macOS derives its cookie key with. */
export const CHROMIUM_KEY_SALT = 'saltysalt';
/** PBKDF2 rounds Chromium on macOS derives its cookie key with. */
export const CHROMIUM_KEY_ROUNDS = 1003;
/** Bytes of Chromium's AES-128 cookie key. */
export const CHROMIUM_KEY_BYTES = 16;
/** The prefix Chromium on macOS puts before an encrypted cookie value. */
export const CHROMIUM_V10 = 'v10';
/** Cookie database version from which a decrypted value starts with a SHA-256 of its host. */
export const CHROMIUM_HASHED_VALUE_VERSION = 24;
/** Bytes of that SHA-256 prefix. */
export const SHA256_BYTES = 32;
/** Milliseconds a person has to answer the Keychain prompt before the import gives up. */
export const KEYCHAIN_PROMPT_TIMEOUT_MS = 120_000;
/** Seconds between the Unix epoch and 2001-01-01, where Safari counts its times from. */
export const MAC_EPOCH_OFFSET_S = 978_307_200;
/** Safari's binary cookie flag for a secure cookie. */
export const SAFARI_SECURE = 1;
/** Safari's binary cookie flag for an HTTP-only cookie. */
export const SAFARI_HTTP_ONLY = 4;
/** Byte offsets inside one Safari binary cookie record. */
export const SAFARI_RECORD = { flags: 8, domain: 16, name: 20, path: 24, value: 28, expiry: 40 } as const;
/** Bytes of a 32-bit field. */
export const UINT32_BYTES = 4;
/** Bytes of a Safari binary cookie page's header, before its cookie count. */
export const SAFARI_PAGE_HEADER = 4;
/** Bytes before a Safari binary cookie file's page-size table: the magic and the page count. */
export const SAFARI_TABLE_START = 8;
/** System Settings' Full Disk Access pane, where a person lets Oya read Safari's files. */
export const FULL_DISK_ACCESS_PANE = 'x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles';
/** What the switch question calls the profile with no server name, as the Ask box's profile menu does. */
export const DEFAULT_PROFILE_NAME = 'Default profile';
/** The switch-profile question's answer that keeps the current profile: the default, and what Escape picks. */
export const STAY_ON_CURRENT = 0;
/** The switch-profile question's answer that moves to the imported profile; a deliberate click, never Enter. */
export const SWITCH_TO_IMPORTED = 1;
