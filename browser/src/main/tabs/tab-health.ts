/**
 * A tab whose page stops responding, and a record of it. A hung renderer goes
 * blank when its tab is shown again and stops its audio, and nothing said so: the
 * tab now says it is not responding, and a log in the user-data folder keeps
 * when each hang, recovery and renderer or helper-process death was detected, so
 * the cause (a peer tab, the agent's tab, the GPU or audio process) can be found.
 */
import type { Details } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { JsonFile } from '../app/json-file.ts';
import { privateSession } from '../native-contexts/index.ts';
import { TAB_HEALTH_ENTRIES } from './constants.ts';
import type { Tab } from './types.ts';

/** What happened: to a tab's page (`probe-failed`: it did not answer a person's Reload), or (`process-gone`) to one of the app's helper processes. */
export type TabHealthEvent = 'probe-failed' | 'gone' | 'process-gone';

/** One line of the log. Tab fields are absent for a helper process, process fields for a tab. */
export interface TabHealthEntry {
  /** When the app noticed, not when it began: a background hang shows only once something waits on the page. */
  detectedAt: string;
  /** What happened. */
  event: TabHealthEvent;
  /** The tab's id. */
  tab?: number;
  /** The page's site only: a full address can carry tokens, and a private tab's is never written. */
  origin?: string;
  /** The renderer's OS process id (0 once it is gone). */
  pid?: number;
  /** Whether an agent held the tab. */
  agent?: boolean;
  /** Other tabs in the same renderer process: a hang in one is a hang in all of them. */
  peers?: number[];
  /** Why a renderer or helper process went away. */
  reason?: string;
  /** Which helper process went away (GPU, Audio Service, …). */
  process?: string;
}

/** What the log reads: every tab, and which ones an agent holds. */
type LogDeps = Pick<AppServices, 'tabs' | 'agentTab'>;

/** The site of the tab's page, or '' for a private tab or an address with none. */
function siteForLog(tab: Tab): string {
  if (privateSession(tab.view.webContents.session)) return '';
  try {
    return new URL(tab.view.webContents.getURL()).origin;
  } catch {
    return '';
  }
}

/** Keeps the newest TAB_HEALTH_ENTRIES health events in one owner-only file. */
export class TabHealthLog {
  /** The file. */
  private readonly file: JsonFile<TabHealthEntry[]>;
  /** The tabs and the agent's claims. */
  private readonly deps: LogDeps;

  /** `file` is the log in the user-data folder; `deps` are the main-process services. */
  constructor(deps: LogDeps, file: JsonFile<TabHealthEntry[]>) {
    this.deps = deps;
    this.file = file;
  }

  /** Records what happened to `tab`'s page, with who else shares its renderer. */
  record(tab: Tab, event: TabHealthEvent, reason?: string): void {
    const pid = tab.view.webContents.getOSProcessId?.() || 0;
    const peers = pid ? this.peersOf(tab, pid) : [];
    const agent = this.deps.agentTab.claimed().includes(tab.id);
    this.append({ event, tab: tab.id, origin: siteForLog(tab), pid, agent, peers, ...(reason ? { reason } : {}) });
  }

  /** Records a helper process (GPU, audio, a utility) that went away: no tab event says so. */
  processGone(details: Details): void {
    this.append({ event: 'process-gone', process: details.name || details.type, reason: details.reason });
  }

  /** The other tabs whose page runs in process `pid`. */
  private peersOf(tab: Tab, pid: number): number[] {
    const others = this.deps.tabs.list.filter((t) => t !== tab && !t.window && !t.view.webContents.isDestroyed());
    return others.filter((t) => t.view.webContents.getOSProcessId?.() === pid).map((t) => t.id);
  }

  /** Adds one entry, keeping the newest; a missing or corrupt file starts over. */
  private append(entry: Omit<TabHealthEntry, 'detectedAt'>): void {
    const saved = this.file.read();
    const entries = Array.isArray(saved) ? saved : [];
    entries.push({ detectedAt: new Date().toISOString(), ...entry });
    this.file.write(entries.slice(-TAB_HEALTH_ENTRIES));
  }
}
