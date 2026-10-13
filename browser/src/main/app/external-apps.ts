/** Human-confirmed handoff to installed desktop apps, with Oya pairing kept internal. */
import type { WebContents, MessageBoxOptions, Event, WebContentsWillFrameNavigateEventParams } from 'electron';
import { OPEN_EXTERNAL_APP_CHOICE } from './constants.ts';
import { externalProtocol, externalAppLink, externalAppName } from './external-app-links.ts';
import { ExternalAppGate, type ExternalAppDeps, type ExternalAppTicket } from './external-app-gate.ts';
export { zoomMeetingLink } from './external-app-links.ts';
/** One pending native confirmation across all tabs prevents popup storms. */
export class ExternalApps {
  /** Main-process collaborators. */
  private readonly deps: ExternalAppDeps;
  /** Admission and document lifetime checks. */
  private readonly gate: ExternalAppGate;
  /** A prompt or OS launch is already in progress. */
  private pending = false;
  /** Navigation listeners are installed at most once per surface. */
  private readonly wired = new WeakSet<WebContents>();
  /** Constructed once, before tabs begin loading. */
  constructor(deps: ExternalAppDeps) {
    this.deps = deps;
    this.gate = new ExternalAppGate(deps);
  }
  /** Include subframes, redirects and popup tabs without injecting page scripts. */
  wire(contents: WebContents): void {
    if (this.wired.has(contents)) return;
    this.wired.add(contents);
    this.gate.watch(contents);
    contents.on('will-frame-navigate', (event) => this.intercept(event, contents));
    contents.on('will-redirect', (event) => this.intercept(event, contents));
  }
  /** Claim app navigation before the renderer replaces the source document. */
  private intercept(event: Event<WebContentsWillFrameNavigateEventParams>, contents: WebContents): void {
    if (this.request(event.url, contents)) event.preventDefault();
  }
  /** Synchronously claim the navigation, then ask asynchronously without changing the page. */
  request(raw: string, contents?: WebContents): boolean {
    const protocol = externalProtocol(raw);
    if (!protocol) return false;
    const link = externalAppLink(raw),
      ticket = this.gate.capture(contents);
    if (!link || !ticket || this.pending) return true;
    this.begin(link, protocol, ticket);
    return true;
  }
  /** Keep one lock across lookup, consent and launch, including every failure path. */
  private begin(link: string, protocol: string, ticket: ExternalAppTicket): void {
    this.pending = true;
    const work = protocol === 'oya:' ? this.deps.deepLinks.applyDeepLink(link) : this.launch(link, ticket);
    void work
      .catch(() => this.failed(ticket))
      .catch(() => {})
      .finally(() => {
        this.pending = false;
      });
  }
  /** The OS identifies the installed handler; page text never supplies an application name. */
  private async handler(link: string): Promise<string> {
    const info = await this.deps.electron.app.getApplicationInfoForProtocol(link).catch(() => null);
    return externalAppName(info?.name || '');
  }
  /** Recheck native context after lookup and the person's affirmative response. */
  private async launch(link: string, ticket: ExternalAppTicket): Promise<void> {
    const name = await this.handler(link);
    if (!this.gate.current(ticket)) return;
    if (!name) return this.missing(ticket);
    const result = await this.ask(ticket, this.confirmation(name, ticket.source));
    if (result.response !== OPEN_EXTERNAL_APP_CHOICE || !this.gate.current(ticket)) return;
    await this.deps.electron.shell.openExternal(link);
  }
  /** Cancellation is the default; only a person's affirmative click can launch. */
  private confirmation(name: string, source: string): MessageBoxOptions {
    return {
      type: 'question',
      message: `Open ${name}?`,
      detail: `${this.sourceLabel(source)} wants to open ${name}.`,
      buttons: ['Cancel', `Open ${name}`],
      ...{ defaultId: 0, cancelId: 0, noLink: true },
    };
  }
  /** Never display meeting passwords, private paths, or query strings in prompts. */
  private sourceLabel(raw: string): string {
    const url = URL.parse(raw);
    return url && ['http:', 'https:'].includes(url.protocol) ? url.origin : 'Oya Browser';
  }
  /** Keep the source page available when no desktop app owns this protocol. */
  private async missing(ticket: ExternalAppTicket): Promise<void> {
    await this.ask(ticket, {
      type: 'info',
      message: 'No app can open this link',
      detail: 'Install the app for this link, then try again.',
      buttons: ['OK'],
    });
  }
  /** Native failures never expose callback tokens or opaque OS exception text. */
  private async failed(ticket: ExternalAppTicket): Promise<void> {
    if (!this.gate.current(ticket)) return;
    await this.ask(ticket, {
      type: 'info',
      message: 'Could not open the app',
      detail: 'Open or reinstall the desktop app, then try this link again.',
      buttons: ['OK'],
    });
  }
  /** Always parent to the requesting window rather than the app's last selected window. */
  private ask(ticket: ExternalAppTicket, options: MessageBoxOptions) {
    const window = ticket.scope.shell.window;
    return window && !window.isDestroyed()
      ? this.deps.electron.dialog.showMessageBox(window, options)
      : this.deps.electron.dialog.showMessageBox(options);
  }
}
