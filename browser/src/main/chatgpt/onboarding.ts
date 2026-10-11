/**
 * First run in under a minute: sign in with ChatGPT (which also makes this app's
 * Oya key when it has none: no Oya account to create), tell the server the model
 * lives here, connect and import the person's logins, and offer to become the
 * default browser. The same `connect` serves an already signed-in browser that
 * just switches its model to the ChatGPT plan.
 */
import type { AppServices } from '../app/services.ts';
import { DefaultBrowser } from '../app/default-browser.ts';
import { ServerApi } from '../connection/server-api.ts';
import { HOME_URL } from '../tabs/constants.ts';
import { signUp as agentSignUp, type SignedUp } from './signup.ts';
import * as C from './constants.ts';
import type { ChatgptAccount } from './auth.ts';

/** The services onboarding uses. */
type Deps = Pick<AppServices, 'config' | 'socket' | 'mirror' | 'chatgpt' | 'tabs' | 'electron' | 'shell'>;

/** The project's model settings once its ChatGPT sign-in lives on this desktop: the server relays every call here. */
const DESKTOP_MODEL = {
  llm_provider: 'chatgpt',
  openai_api_key: C.DESKTOP_HELD,
  openai_base_url: null,
  chat_model: null,
};

/** How the person asked to connect. */
export interface ConnectOptions {
  /** Bring in the default browser's logins once connected. */
  importLogins: boolean;
}

/** What the checklist shows. */
export interface OnboardingStatus {
  /** This browser has an Oya key. */
  signedIn: boolean;
  /** The ChatGPT sign-in, and as whom. */
  chatgpt: ChatgptAccount;
  /** The person's logins came in from another browser. */
  imported: boolean;
  /** Oya Browser is the system's default browser. */
  isDefault: boolean;
  /** Where the person claims this browser's key into an Oya account, when it has none yet. */
  claimUrl: string;
}

/** The first-run flow and its state. */
export class Onboarding {
  /** True from a first-run connect until the person starts browsing: the checklist stays up meanwhile. */
  active = false;
  /** The main-process services. */
  private readonly deps: Deps;
  /** The server's HTTP API. */
  private readonly api: ServerApi;
  /** The system default-browser setting. */
  private readonly defaultBrowser: DefaultBrowser;
  /** The agent sign-up, replaceable in tests. */
  private readonly signUp: (serverUrl: string, email: string) => Promise<SignedUp>;

  /** Runs over `deps`; `signUp` stands in for the server's agent sign-up in tests. */
  constructor(deps: Deps, signUp = agentSignUp) {
    this.deps = deps;
    this.api = new ServerApi(deps);
    this.defaultBrowser = new DefaultBrowser(deps);
    this.signUp = (url, email) => signUp(url, email);
  }

  /** Signs in with ChatGPT and puts the project's model on it; a browser without a key gets one and connects. */
  async connect({ importLogins }: ConnectOptions): Promise<ChatgptAccount> {
    const account = await this.deps.chatgpt.signIn();
    const minted = await this.ensureKey(account.email);
    await this.api.post('config', DESKTOP_MODEL);
    if (minted) this.connectFirstTime(importLogins);
    return account;
  }

  /** Makes this browser's key from the ChatGPT email when it has none; answers whether it did. */
  private async ensureKey(email: string): Promise<boolean> {
    const { config } = this.deps;
    if (config.values.apiKey) return false;
    if (!email) throw Error(C.NO_EMAIL);
    const { apiKey, claimUrl } = await this.signUp(config.values.serverUrl, email);
    config.merge({ apiKey, claimUrl, keyFromApp: true, signedOut: false, account: null });
    config.save();
    return true;
  }

  /** Connects with the new key, holding the checklist up, and imports the logins once connected when asked. */
  private connectFirstTime(importLogins: boolean): void {
    this.active = true;
    this.deps.mirror.importOnConnect = importLogins;
    this.deps.socket.browserId = null;
    this.deps.socket.connect();
  }

  /** Where each step stands. */
  status(): OnboardingStatus {
    const { apiKey, mirroredFrom, imports, claimUrl } = this.deps.config.values;
    return {
      ...{ signedIn: !!apiKey, chatgpt: this.deps.chatgpt.status() },
      ...{ imported: !!mirroredFrom || (Array.isArray(imports) && imports.length > 0) },
      ...{ isDefault: this.defaultBrowser.isDefault(), claimUrl: String(claimUrl || '') },
    };
  }

  /** Done: the checklist goes and the browser opens on the start page. */
  finish(): void {
    this.active = false;
    this.deps.tabs.enterBrowsingMode(HOME_URL);
  }
}
