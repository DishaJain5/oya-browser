/**
 * The first-run checklist: connect ChatGPT, or a free Gemini key with an email,
 * (either also makes this browser's Oya key, so there is no account to create), bring in the person's logins, make Oya
 * the default browser, and start. Each step's state is read from the main
 * process, so it is right whichever order things happen in.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import { shape } from '../model/models.ts';

/** Where each step stands, as the main process reports it. */
export interface OnboardingProgress {
  /** This browser has an Oya key. */
  signedIn: boolean;
  /** The ChatGPT sign-in, and as whom. */
  chatgpt: { /** A sign-in is kept. */ connected: boolean; /** Its email. */ email: string };
  /** The project has a model: the ChatGPT plan or a Gemini key. */
  hasModel: boolean;
  /** The person's logins came in. */
  imported: boolean;
  /** Oya Browser is the default browser. */
  isDefault: boolean;
  /** Where the key becomes an Oya account. */
  claimUrl: string;
}

/** Step 1's ways in, one per tab of its switch. */
export type ConnectWay = 'chatgpt' | 'gemini' | 'oya' | 'key';

/** What the checklist shows. */
export interface ChecklistState {
  /** The steps, once read. */
  progress: OnboardingProgress | null;
  /** The ChatGPT sign-in is under way. */
  connecting: boolean;
  /** Bring in the default browser's logins on connect. */
  importLogins: boolean;
  /** The way in step 1 shows: ChatGPT, the Gemini key form, an Oya account, or an Oya API key. */
  way: ConnectWay;
  /** The email typed in the key form. */
  email: string;
  /** The Gemini key typed in the key form. */
  key: string;
  /** Why the last step failed, or ''. */
  error: string;
}

/** The parts of the bridge the checklist uses. */
export type ChecklistBridge = Pick<
  OyaBrowser,
  | 'connectChatgpt'
  | 'connectGemini'
  | 'openGeminiKeys'
  | 'onboardingStatus'
  | 'finishOnboarding'
  | 'makeDefaultBrowser'
  | 'onWsStatus'
  | 'onMirrorStatus'
>;

/** The first-run checklist. */
export class ChecklistViewModel extends ViewModel<ChecklistState> {
  /** The main process. */
  private readonly bridge: ChecklistBridge;

  /** Reads the steps now, and again whenever the connection or an import changes. */
  constructor(bridge: ChecklistBridge) {
    super({ progress: null, connecting: false, importLogins: true, way: 'oya', email: '', key: '', error: '' });
    this.bridge = bridge;
    this.own(bridge.onWsStatus(() => void this.refresh()));
    this.own(bridge.onMirrorStatus(() => void this.refresh()));
    void this.refresh();
  }

  /** Re-reads where each step stands. */
  async refresh(): Promise<void> {
    const progress = shape<OnboardingProgress>(await this.bridge.onboardingStatus().catch(() => null));
    if (progress.chatgpt) this.set({ progress });
  }

  /** Ticks or unticks "bring in my logins". */
  toggleImport(importLogins: boolean): void {
    this.set({ importLogins });
  }

  /** Step 1: ChatGPT sign-in in the web browser; the rest follows on its own. */
  async connectChatgpt(): Promise<void> {
    this.set({ connecting: true, error: '' });
    const answer = await this.bridge
      .connectChatgpt({ importLogins: this.state.importLogins })
      .catch((e: Error) => ({ error: e.message }));
    this.set({ connecting: false, error: String(answer?.error || '') });
    await this.refresh();
  }

  /** Switches step 1 to another way in. */
  pick(way: ConnectWay): void {
    this.set({ way, error: '' });
  }

  /** Records what is typed in the key form. */
  type(fields: Partial<Pick<ChecklistState, 'email' | 'key'>>): void {
    this.set(fields);
  }

  /** Opens Google's free-key page in the system browser. */
  getKey(): void {
    void this.bridge.openGeminiKeys();
  }

  /** Step 1 without ChatGPT: the email makes the Oya key and the project runs on the Gemini key. */
  async connectGemini(): Promise<void> {
    const { email, key, importLogins } = this.state;
    this.set({ connecting: true, error: '' });
    const answer = await this.bridge
      .connectGemini({ email, key, importLogins })
      .catch((e: Error) => ({ error: e.message }));
    this.set({ connecting: false, error: String(answer?.error || '') });
    await this.refresh();
  }

  /** Step 3: the system's default-browser choice. */
  async makeDefault(): Promise<void> {
    await this.bridge.makeDefaultBrowser();
    await this.refresh();
  }

  /** Step 4: start browsing. */
  finish(): void {
    void this.bridge.finishOnboarding();
  }
}
