/** Invalidate asynchronous highlight reads when their document, visible tab or activity is replaced. */
import type { WebContents } from 'electron';

/** One shield's current document and generation; lifecycle events never carry geometry between tabs. */
export class HighlightEpoch {
  /** Generation captured before asynchronous page reads. */
  revision = 0;
  /** The document surface currently beneath this shield. */
  private contents: WebContents | null = null;
  /** Removes old geometry from the overlay when its document changes. */
  private readonly clear: () => void;
  /** A navigation, crash or destruction invalidates all pending measurements. */
  private readonly gone = () => this.clear();
  /** Only a top-level navigation replaces the measured page. */
  private readonly navigating = (event: unknown, _url: string, _sameDocument: boolean, main: boolean) => {
    const details = event as { /** Current Electron navigation ownership. */ isMainFrame?: boolean } | null;
    if (details?.isMainFrame ?? main) this.gone();
  };

  /** Receives the shield's visual reset without depending on its implementation. */
  constructor(clear: () => void) {
    this.clear = clear;
  }

  /** Cancels pending reads, including ones whose page still has the same URL. */
  bump(): void {
    this.revision += 1;
  }

  /** Watches only the shown page and detaches listeners when switching tabs or removing the shield. */
  watch(contents: WebContents | null): void {
    if (this.contents === contents) return;
    this.listen('removeListener');
    this.contents = contents;
    this.clear();
    this.listen('on');
  }

  /** Installs or removes the exact listeners belonging to the current page. */
  private listen(method: 'on' | 'removeListener'): void {
    this.contents?.[method]('did-start-navigation', this.navigating);
    this.contents?.[method]('render-process-gone', this.gone);
    this.contents?.[method]('destroyed', this.gone);
  }
}
