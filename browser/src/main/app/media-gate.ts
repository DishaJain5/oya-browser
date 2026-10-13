/**
 * Who may ask a page's device question, and the dialog it is asked with. The
 * one place microphone, camera and screen share all go through, so a grant
 * remembered on disk can never reach a tab an agent drives, a governed
 * browser, a background tab or an insecure page: the gate runs before any
 * store is read, and again after every await.
 */
import type { WebContents, MessageBoxOptions, MessageBoxReturnValue } from 'electron';
import type { AppServices } from './services.ts';
import { OS_DENIED_PROMPT } from './media-prompts.ts';

/** Sensitive access uses native dialogs and the normal control boundary. */
type Deps = Pick<AppServices, 'electron' | 'shell' | 'tabs' | 'agentTab' | 'governance'>;

/** HTTPS origins are displayed rather than private paths and query strings. */
export function mediaOrigin(raw: string): string | null {
  const url = URL.parse(raw);
  return url?.protocol === 'https:' && !url.username && !url.password ? url.origin : null;
}

/** The rules every device question passes, and the one dialog at a time it is asked in. */
export class MediaGate {
  /** Native APIs, active tab and control policy. */
  private readonly deps: Deps;
  /** Navigation generations fence reloads even when the URL does not change. */
  private readonly generations = new WeakMap<WebContents, number>();
  /** The document generation the macOS denial was last explained for: once per page, not once per request. */
  private readonly explained = new WeakMap<WebContents, number>();
  /** Prevents a page from stacking permission dialogs. */
  private pending = false;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Refuses background tabs, a tab an agent drives, and managed runtimes. Per tab: with agents connected, the person still owns the tabs no agent claimed. */
  allowed(contents: WebContents | null): contents is WebContents {
    return (
      !!contents &&
      !contents.isDestroyed() &&
      !this.deps.governance.configuration &&
      this.deps.agentTab.humanMay(this.deps.tabs.activeTabId) &&
      this.deps.tabs.getActiveView()?.webContents === contents
    );
  }

  /** The requesting origin, when `contents` may ask and `raw` is the same secure document; null otherwise. */
  eligible(contents: WebContents | null, raw: string | undefined): string | null {
    if (!this.allowed(contents) || !raw) return null;
    const origin = mediaOrigin(raw);
    return origin && origin === mediaOrigin(contents.getURL()) ? origin : null;
  }

  /** Keeps a single navigation listener per tab. Only a new top document revokes: a pushState or an iframe loading keeps it. Answers the document's generation. */
  watch(contents: WebContents): number {
    if (!this.generations.has(contents)) {
      this.generations.set(contents, 0);
      contents.on('did-start-navigation', (e) => e.isMainFrame && !e.isSameDocument && this.revoke(contents));
    }
    return this.generations.get(contents)!;
  }

  /** A new top document, including a same-URL reload. */
  private revoke(contents: WebContents): void {
    this.generations.set(contents, this.generations.get(contents)! + 1);
  }

  /** A prompt cannot grant a replacement document, or a tab an agent took meanwhile. */
  current(contents: WebContents, generation: number): boolean {
    return this.allowed(contents) && this.generations.get(contents) === generation;
  }

  /** Runs `prompt` as the one dialog up; a request while one is up is refused, not queued. Always settles to a boolean. */
  locked(prompt: () => Promise<boolean>): Promise<boolean> {
    if (this.pending) return Promise.resolve(false);
    this.pending = true;
    return prompt()
      .catch(() => false)
      .finally(() => void (this.pending = false));
  }

  /** Whether the person clicked Allow. */
  async allowedBy(options: MessageBoxOptions, choice: number): Promise<boolean> {
    return (await this.ask(options)).response === choice;
  }

  /** Explains the OS denial once per document; always answers false. */
  async osDenied(contents: WebContents, generation: number): Promise<false> {
    if (this.explained.get(contents) !== generation) {
      this.explained.set(contents, generation);
      await this.ask(OS_DENIED_PROMPT);
    }
    return false;
  }

  /** Parents to Oya when possible; never passes Electron a null window. */
  ask(options: MessageBoxOptions): Promise<MessageBoxReturnValue> {
    const window = this.deps.shell.window;
    return window && !window.isDestroyed()
      ? this.deps.electron.dialog.showMessageBox(window, options)
      : this.deps.electron.dialog.showMessageBox(options);
  }
}
