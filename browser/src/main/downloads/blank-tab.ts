/**
 * A tab opened only to download a file (a link with target="_blank", a typed
 * file address) is left blank once the download takes over. Chrome closes it;
 * so does Oya, but only a tab that never showed a page.
 */
import type { WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';

/** Addresses of a tab that never showed a page. */
const BLANK = new Set(['', 'about:blank']);

/** Closes the tab whose page is `contents` when it never showed a page. */
export function closeDownloadOnlyTab(deps: Pick<AppServices, 'tabs'>, contents: WebContents | undefined): void {
  const tab = deps.tabs.list.find((t) => t.view.webContents === contents);
  if (!tab || tab.home || !contents || contents.isDestroyed()) return;
  if (BLANK.has(contents.getURL()) && !contents.navigationHistory.canGoBack()) deps.tabs.closeTab(tab.id);
}
