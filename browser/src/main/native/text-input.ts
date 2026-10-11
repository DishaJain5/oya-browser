/** Acknowledged native composition targets the owned widget without moving OS focus or disabling the shield. */
import type { WebContents } from 'electron';
/** Explicit engine capability prevents older runtimes silently dropping hidden-widget text. */
export interface NativeTextPage {
  /** Exact native page, never whichever browser window happens to be focused. */
  webContents: Pick<WebContents, 'isDestroyed' | 'insertText' | 'isLoadingMainFrame'> & {
    /** Document-scoped native IME acknowledgement with deadline and focus restoration. */
    _insertTextOya?: (text: string) => Promise<void>;
  };
}
/** Do not substitute script assignment, synthetic input events, or debugging transport. */
export async function insertNativeText(page: NativeTextPage, text: string): Promise<object> {
  const contents = page.webContents;
  if (contents.isDestroyed()) throw Error('View is destroyed');
  if (contents._insertTextOya) await contents._insertTextOya(text);
  else await insertStockText(contents, text);
  return {};
}
/** Stock Electron (desktop) has no document-bound insertion: refuse while the document is being replaced. */
async function insertStockText(contents: NativeTextPage['webContents'], text: string): Promise<void> {
  if (contents.isLoadingMainFrame()) throw Error('Text input refused while the page is navigating');
  await contents.insertText(text);
}
