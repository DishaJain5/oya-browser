/**
 * Keeps agents' pages rendering while the person shows another tab in the same
 * window. Stock Electron stops painting a view that was never mounted, and refuses to
 * capture it, so an agent's screenshots and the live view would go blank. Each
 * agent's view stays mounted beneath the shown one, with its bounds; over the start
 * page (the shell itself, not a view) they are taken off, which is safe once mounted.
 */
import type { BrowserView, BrowserWindow } from 'electron';
import type { AppServices } from '../app/services.ts';

/** What the stack reads: the agents' tabs and this window's tabs. */
type Deps = Pick<AppServices, 'agentTab' | 'tabs'>;

/** The agents' views this window holds but does not show. */
function hiddenAgentViews(deps: Deps): BrowserView[] {
  if (!deps.agentTab.scoped()) return [];
  const claimed = deps.agentTab.claimed();
  const shown = deps.tabs.getShownView();
  const views = deps.tabs.list.filter((t) => claimed.includes(t.id) && !t.window).map((t) => t.view as BrowserView);
  return views.filter((view) => view !== shown && !view.webContents.isDestroyed());
}

/**
 * Shows only `view` (or no view, over the start page). Electron 44's own
 * setBrowserView removes the old views while walking the same list it removes
 * from, so every other one stays on: a stale page then shows under a new tab,
 * with its old bounds. getBrowserViews() hands back a copy, so this walk is safe.
 */
export function mountOnly(win: BrowserWindow, view: BrowserView | null): void {
  for (const mounted of win.getBrowserViews()) if (mounted !== view) win.removeBrowserView(mounted);
  if (view && !win.getBrowserViews().includes(view)) win.addBrowserView(view);
}

/** Mounts the agents' views beneath the shown view with its bounds, or takes them off when nothing covers them. */
export function keepAgentRendering(deps: Deps, win: BrowserWindow): void {
  const agents = hiddenAgentViews(deps);
  const shown = deps.tabs.getShownView() as BrowserView | null;
  if (!shown) return agents.forEach((agent) => win.removeBrowserView(agent));
  for (const agent of agents) agent.setBounds(shown.getBounds());
  if (agents.length) stackBeneath(win, agents);
}

/** Puts `agents` at the bottom of the window's views, the rest above in their order; nothing moves when they already are. */
function stackBeneath(win: BrowserWindow, agents: BrowserView[]): void {
  const views = win.getBrowserViews();
  if (agents.every((agent, i) => views.indexOf(agent) === i)) return;
  for (const agent of agents) if (!views.includes(agent)) win.addBrowserView(agent);
  for (const view of views.filter((v) => !agents.includes(v))) win.setTopBrowserView(view);
}
