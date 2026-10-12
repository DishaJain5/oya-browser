/**
 * This app's ChatGPT sign-in: it opens OpenAI's page in the person's own browser,
 * takes the code on a loopback address, keeps the credential sealed on this
 * machine, and hands out a live access token, refreshing it one request at a time
 * (each refresh rotates the refresh token, so two at once would race). The token
 * never leaves this machine except to OpenAI.
 */
import { randomUUID } from 'node:crypto';
import type { AppServices } from '../app/services.ts';
import { SealedJson } from './credential-store.ts';
import { awaitCallback, type Callback } from './loopback.ts';
import * as O from './oauth.ts';
import * as C from './constants.ts';
import { openOutside } from './outside.ts';

/** The services sign-in uses. */
type Deps = Pick<AppServices, 'electron'>;

/** What tests replace: the network, the listener, and where the sealed file goes. */
export interface AuthSeams {
  /** fetch, for the token endpoint. */
  fetch?: O.Fetch;
  /** Starts the loopback listener. */
  listen?: () => Promise<Callback>;
  /** The sealed credential. */
  store?: SealedJson<O.Credential>;
}

/** Whether this app holds a ChatGPT sign-in, and as whom. */
export interface ChatgptAccount {
  /** A sign-in is kept. */
  connected: boolean;
  /** The account's email, or ''. */
  email: string;
}

/** The ChatGPT account this app is signed in to. */
export class ChatgptAuth {
  /** The main-process services. */
  private readonly deps: Deps;
  /** The network. */
  private readonly fetch: O.Fetch;
  /** The loopback listener. */
  private readonly listen: () => Promise<Callback>;
  /** The credential at rest. */
  private readonly store: SealedJson<O.Credential>;
  /** The refresh in flight, so concurrent callers share one. */
  private refreshing: Promise<O.Credential> | null = null;

  /** Signs in through `deps`; `seams` stand in for the network and disk in tests. */
  constructor(deps: Deps, seams: AuthSeams = {}) {
    this.deps = deps;
    this.fetch = seams.fetch ?? fetch;
    this.listen = seams.listen ?? (() => awaitCallback());
    const dir = deps.electron.app.getPath('userData');
    this.store = seams.store ?? new SealedJson(dir, C.CREDENTIAL_FILE, deps.electron.safeStorage);
  }

  /** Signs in: OpenAI's page in the person's usual browser (never Oya itself), then the code exchanged and sealed. Answers the account. */
  async signIn(): Promise<ChatgptAccount> {
    const callback = await this.listen();
    try {
      const credential = await this.through(callback);
      this.store.save(credential);
      return { connected: true, email: credential.email || '' };
    } finally {
      callback.close();
    }
  }

  /** Opens the sign-in for `callback`'s address and waits for its credential. */
  private async through(callback: Callback): Promise<O.Credential> {
    // Every sign-in registers afresh under a new host id, as the reference flow does: reusing an
    // earlier registration or client made OpenAI refuse the code exchange.
    const login = O.newLogin(callback.redirectUri);
    await openOutside(this.deps, O.authorizeUrl(login, `urn:uuid:${randomUUID()}`));
    return this.complete(login, await callback.done);
  }

  /** Checks what came back and exchanges the code, never under the registration client. */
  private async complete(login: O.Login, query: Record<string, string>): Promise<O.Credential> {
    if (query.state !== login.state) throw Error('ChatGPT sign-in did not match. Try again.');
    if (query.error) throw Error('ChatGPT sign-in was cancelled or not allowed.');
    const clientId = query.client_id || login.clientId;
    if (clientId === C.DYNAMIC_CLIENT) throw Error('ChatGPT did not finish the sign-in. Try again.');
    const tokens = await O.exchangeCode(login, clientId, query.code || '', this.fetch);
    return O.credentialFrom(tokens, clientId, login.nonce);
  }

  /** A live access token, refreshed when it is close to expiring; throws when there is no sign-in. */
  async accessToken(): Promise<string> {
    const credential = this.store.load();
    if (!credential) throw Error('Not signed in to ChatGPT');
    if (credential.access_token && (credential.expires_at || 0) - Date.now() > C.REFRESH_EARLY_MS)
      return credential.access_token;
    return String((await this.refresh(credential)).access_token);
  }

  /** One refresh at a time; the rotated credential is sealed before anyone uses it. */
  private refresh(credential: O.Credential): Promise<O.Credential> {
    this.refreshing ??= O.refreshed(credential, this.fetch)
      .then((next) => (this.store.save(next), next))
      .finally(() => (this.refreshing = null));
    return this.refreshing;
  }

  /** Whether this app is signed in to ChatGPT, and as whom. */
  status(): ChatgptAccount {
    const credential = this.store.load();
    return { connected: !!credential, email: credential?.email || '' };
  }

  /** Forgets the sign-in. */
  signOut(): void {
    this.store.clear();
  }
}
