/** Local suggestions with stale-response fencing and ordered native-overlay ownership. */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { AddressSuggestion } from '../../../../shared/ipc.ts';
import { ADDRESS_OVERLAY } from '../model/constants.ts';
/** Only local queries and this feature's overlay are used. */
type Bridge = Pick<OyaBrowser, 'addressSuggestions' | 'showOverlay' | 'hideOverlay'>;
/** The combobox's visible options and active descendant. */
interface State {
  /** Suggestions, in relevance order. */
  items: AddressSuggestion[];
  /** Selected row; -1 keeps the typed query as the Enter action. */
  selected: number;
  /** Whether the dropdown is visible. */
  open: boolean;
  /** The rest of the best match, shown selected after what was typed (Chrome's inline completion); '' for none. */
  inline: string;
}

/**
 * What to complete `query` with: the rest of the best suggestion's address, when that
 * address (without scheme or www.) starts with exactly what was typed; a home page
 * loses its trailing slash.
 */
export function inlineFor(query: string, best: AddressSuggestion | undefined): string {
  const address = best?.url.replace(/^https?:\/\/(www\.)?/i, '').replace(/^([^/?#]+)\/$/, '$1') ?? '';
  return query && address.toLowerCase().startsWith(query.toLowerCase()) ? address.slice(query.length) : '';
}
/** UI state stays out of the main-process library service. */
export class AddressCompletion extends ViewModel<State> {
  /** The trusted shell bridge. */
  private readonly bridge: Bridge;
  /** Changes on every query, dismissal and disposal. */
  private revision = 0;
  /** Serializes show/hide so late screenshot completion cannot strand the page. */
  private overlay = Promise.resolve();
  /** Starts closed, without exposing history merely because a page loads. */
  constructor(bridge: Bridge) {
    super({ items: [], selected: -1, open: false, inline: '' });
    this.bridge = bridge;
  }
  /** Local IPC may answer out of order; only the newest edit may show suggestions. */
  /** `typing` is false for a deletion, which never completes inline, so Backspace can take the text back. */
  async search(query: string, typing = false): Promise<void> {
    const revision = ++this.revision;
    if (!query.trim()) return this.dismiss();
    this.set({ items: [], selected: -1, inline: '' });
    const found = await this.bridge.addressSuggestions(query).catch(() => []);
    if (revision !== this.revision) return;
    const items = Array.isArray(found) ? found : [];
    this.set({ items, selected: -1, inline: typing ? inlineFor(query, items[0]) : '' });
    this.visibility(items.length > 0);
  }
  /** Arrow keys wrap without changing the user's original query. */
  move(offset: number): void {
    const count = this.state.items.length;
    if (!count) return;
    const start = this.state.selected < 0 && offset < 0 ? 0 : this.state.selected;
    this.set({ selected: (start + offset + count) % count, inline: '' });
  }
  /** Enter uses the selected destination, or the original typed URL/search. */
  get selectedUrl(): string | undefined {
    return this.state.items[this.state.selected]?.url;
  }
  /** Blur, Escape and tab changes cancel even an in-flight query. */
  dismiss(): void {
    this.revision++;
    this.set({ items: [], selected: -1, inline: '' });
    this.visibility(false);
  }
  /** Only actual open/close transitions change the native view. */
  private visibility(open: boolean): void {
    if (this.state.open === open) return;
    this.set({ open });
    this.overlay = this.overlay.catch(() => {}).then(() => this.syncOverlay());
  }
  /** Await an in-progress show before a hide, retaining other overlays' ownership. */
  private async syncOverlay(): Promise<void> {
    try {
      if (this.state.open) await this.bridge.showOverlay(ADDRESS_OVERLAY);
      else await this.bridge.hideOverlay(ADDRESS_OVERLAY);
    } catch {
      this.dismiss();
    }
  }
  /** Dispose cannot leave suggestions above a restored browser session. */
  override dispose(): void {
    this.dismiss();
    super.dispose();
  }
}
