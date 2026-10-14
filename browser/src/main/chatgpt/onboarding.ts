/**
 * First run in under a minute: sign in with ChatGPT (which also makes this app's
 * Oya key when it has none: no Oya account to create), tell the server the model
 * lives here, connect and import the person's logins, and offer to become the
 * default browser. The same `connect` serves an already signed-in browser that
 * just switches its model to the ChatGPT plan. Without a ChatGPT plan,
 * `connectWithKey` does the same from a typed email and a Gemini API key.
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

/** The project's model settings for a Gemini key typed at first run; the server's catalog has Gemini's endpoint and model. */
const geminiModel = (key: string) => ({
  llm_provider: 'gemini',
  openai_api_key: key,
  openai_base_url: null,
  chat_model: null,
});

/** How the person asked to connect. */
export interface ConnectOptions {
  /** Bring in the default browser's logins once connected. */
  importLogins: boolean;
}

/** The key route's input: the email the Oya key is made from, and the Gemini key the project runs on. */
export interface KeyConnectOptions extends ConnectOptions {
  /** The person's email. */
  email: string;
  /** Their Gemini API key. */
  key: string;
}

/** What the checklist shows. */
export interface OnboardingStatus {
  /** This browser has an Oya key. */
  signedIn: boolean;
  /** The ChatGPT sign-in, and as whom. */
  chatgpt: ChatgptAccount;
  /** The project has a model: the ChatGPT plan, or a key given at first run. */
  hasModel: boolean;
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
  /** A ChatGPT sign-in that worked while a later step failed: a retry starts from it, not from OpenAI's page again. */
  private signedIn: ChatgptAccount | null = null;
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
    const account = (this.signedIn ??= await this.signIn());
    const minted = await this.ensureKey(account.email);
    await this.api.post('config', DESKTOP_MODEL);
    this.signedIn = null;
    if (minted) this.connectFirstTime(importLogins);
    return account;
  }

  /** The key route: makes this browser's Oya key from `email` when it has none and puts the project on the Gemini `key`. */
  async connectWithKey({ email, key, importLogins }: KeyConnectOptions): Promise<void> {
    const [mail, secret] = [String(email || '').trim(), String(key || '').trim()];
    if (!C.EMAIL_SHAPE.test(mail)) throw Error(C.BAD_EMAIL);
    if (!secret) throw Error(C.NO_GEMINI_KEY);
    const minted = await this.ensureKey(mail);
    await this.api.post('config', geminiModel(secret));
    this.deps.config.merge({ modelFromKey: true });
    this.deps.config.save();
    if (minted) this.connectFirstTime(importLogins);
  }

  /** Signs in to ChatGPT, then brings Oya back in front of the browser the sign-in ran in. */
  private async signIn(): Promise<ChatgptAccount> {
    const account = await this.deps.chatgpt.signIn();
    this.deps.shell.window?.show();
    return account;
  }

  /** Makes this browser's key from the ChatGPT email when it has none; answers whether it did. */
  private async ensureKey(email: string): Promise<boolean> {
    const { config } = this.deps;
    if (config.values.apiKey) return false;
    if (!email) throw Error(C.NO_EMAIL);
    const { apiKey, claimUrl } = await this.signUp(config.values.serverUrl, email);
    config.merge({ apiKey, claimUrl, keyFromApp: true, signedOut: false, account: null, modelFromKey: false });
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
    const { apiKey, mirroredFrom, imports, claimUrl, modelFromKey } = this.deps.config.values;
    const chatgpt = this.deps.chatgpt.status();
    return {
      ...{ signedIn: !!apiKey, chatgpt, hasModel: chatgpt.connected || !!modelFromKey },
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
