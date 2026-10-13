/** Bind desktop handoff consent to one human-owned tab, window, profile and document. */
import type { WebContents } from 'electron';
import type { AppServices } from './services.ts';
/** Only native ownership and persona services participate in admission. */
export type ExternalAppDeps = Pick<
  AppServices,
  'electron' | 'shell' | 'tabs' | 'agentTab' | 'governance' | 'deepLinks' | 'windows' | 'persona'
>;
/** The context that must survive every asynchronous lookup and confirmation. */
export interface ExternalAppTicket {
  /** Actual window owner, never whichever window became active later. */
  scope: ExternalAppDeps;
  /** Native source surface. */
  contents: WebContents;
  /** Selected tab at admission, including when the address bar initiated the request. */
  tabId: number | null;
  /** Profile binding at admission. */
  partition: string;
  /** Source URL is checked as well as document generation. */
  source: string;
  /** Same-URL reloads still invalidate the ticket. */
  generation: number;
}
/** A shared service can admit a source in any registered browser window. */
export class ExternalAppGate {
  /** Ownership collaborators. */
  private readonly deps: ExternalAppDeps;
  /** Document generations survive URL reuse and listeners are installed once. */
  private readonly generations = new WeakMap<WebContents, number>();
  /** Services are supplied by the composition root. */
  constructor(deps: ExternalAppDeps) {
    this.deps = deps;
  }
  /** Resolve the source window from the exact native surface, including the trusted shell. */
  private owner(contents: WebContents): ExternalAppDeps | undefined {
    if (!this.deps.windows) return this.deps;
    return this.deps.windows.ownerOfContents(contents) || this.deps.windows.fromContents(contents);
  }
  /** Only the selected human-owned surface in an unmanaged desktop can ask. */
  private allowed(contents: WebContents, scope?: ExternalAppDeps): scope is ExternalAppDeps {
    if (!scope || contents.isDestroyed() || scope.governance.configuration) return false;
    if (!scope.agentTab.humanMay(scope.tabs.activeTabId)) return false;
    return contents === scope.tabs.getActiveView()?.webContents || contents === scope.shell.window?.webContents;
  }
  /** A new top-level document revokes pending consent even if its URL does not change. */
  watch(contents: WebContents): number {
    if (!this.generations.has(contents)) {
      this.generations.set(contents, 0);
      contents.on('did-start-navigation', (e) => {
        if (e.isMainFrame && !e.isSameDocument) this.generations.set(contents, this.generations.get(contents)! + 1);
      });
    }
    return this.generations.get(contents)!;
  }
  /** Capture a ticket only after resolving the source's own window and human ownership. */
  capture(contents?: WebContents): ExternalAppTicket | null {
    if (!contents) return null;
    const scope = this.owner(contents);
    if (!this.allowed(contents, scope)) return null;
    const tabId = scope.tabs.activeTabId,
      partition = scope.persona.partitionName();
    const source = contents.getURL(),
      generation = this.watch(contents);
    return { scope, contents, tabId, partition, source, generation };
  }
  /** Never let consent follow a transferred tab, replacement document or changed profile. */
  current(ticket: ExternalAppTicket): boolean {
    const { scope, contents } = ticket;
    return (
      this.owner(contents) === scope &&
      this.allowed(contents, scope) &&
      scope.tabs.activeTabId === ticket.tabId &&
      scope.persona.partitionName() === ticket.partition &&
      this.sameDocument(ticket)
    );
  }
  /** A reload cannot inherit consent just because its address is unchanged. */
  private sameDocument(ticket: ExternalAppTicket): boolean {
    return ticket.contents.getURL() === ticket.source && this.generations.get(ticket.contents) === ticket.generation;
  }
}
