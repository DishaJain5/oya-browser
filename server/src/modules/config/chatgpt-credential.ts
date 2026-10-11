/**
 * What a Sign in with ChatGPT credential looks like at rest: the project's
 * openai_api_key holds it as JSON. Shared by the settings service (which must
 * never let it leave as a plain key) and the sign-in itself.
 */

/** What is kept for a key signed in with ChatGPT. */
export interface Credential {
  /** The client id ChatGPT issued this host for the account; reused on every later sign-in. */
  client_id: string;
  /** The latest refresh token; each refresh replaces it. */
  refresh_token: string;
  /** The current access token, when it fit in storage. */
  access_token?: string;
  /** When the access token expires, ms since the epoch. */
  expires_at?: number;
  /** The ChatGPT account's email, to show which account is connected. */
  email?: string;
}

/**
 * What a key's openai_api_key holds when the ChatGPT credential lives on the
 * person's desktop app instead: the server relays each model call there and never
 * sees a token. The desktop stores the same word (browser/src/main/chatgpt).
 */
export const DESKTOP_HELD = 'chatgpt-desktop';

/** Whether a stored model key says the credential is on a desktop. */
export const desktopHeld = (text: string) => text === DESKTOP_HELD;

/** The credential stored as a key's openai_api_key, or null when it is not one. */
export function credentialOf(text: string): Credential | null {
  try {
    const value = JSON.parse(text);
    return value?.client_id && value?.refresh_token ? value : null;
  } catch {
    return null;
  }
}
