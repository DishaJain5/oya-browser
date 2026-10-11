/**
 * The first-run checklist: connect ChatGPT (which also makes this browser's Oya
 * key, so there is no account to create), bring in the person's logins, make Oya
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
  /** The person's logins came in. */
  imported: boolean;
  /** Oya Browser is the default browser. */
  isDefault: boolean;
  /** Where the key becomes an Oya account. */
  claimUrl: string;
}

/** What the checklist shows. */
export interface ChecklistState {
  /** The steps, once read. */
  progress: OnboardingProgress | null;
  /** The ChatGPT sign-in is under way. */
  connecting: boolean;
  /** Bring in the default browser's logins on connect. */
  importLogins: boolean;
  /** Why the last step failed, or ''. */
  error: string;
}

/** The parts of the bridge the checklist uses. */
export type ChecklistBridge = Pick<
  OyaBrowser,
  'connectChatgpt' | 'onboardingStatus' | 'finishOnboarding' | 'makeDefaultBrowser' | 'onWsStatus' | 'onMirrorStatus'
>;

/** The first-run checklist. */
export class ChecklistViewModel extends ViewModel<ChecklistState> {
  /** The main process. */
  private readonly bridge: ChecklistBridge;

  /** Reads the steps now, and again whenever the connection or an import changes. */
  constructor(bridge: ChecklistBridge) {
    super({ progress: null, connecting: false, importLogins: true, error: '' });
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
