/**
 * The profile card under the toolbar's profile circle, as in Chrome: who is
 * signed in, the project's browsing profiles with the active one ticked, and
 * Settings. The list and the switch are the Ask picker's (PersonaViewModel);
 * this owns only the card's open state and its overlay, which takes the page's
 * view away while the card is up so the page cannot paint over it.
 */
import { ViewModel } from '../../core/view-model.ts';
import type { OyaBrowser } from '../../core/bridge.ts';
import type { PersonaViewModel } from '../ask/index.ts';
import { PROFILES_OVERLAY } from './constants.ts';

/** The parts of the bridge the card uses. */
type Bridge = Pick<OyaBrowser, 'showOverlay' | 'hideOverlay'>;

/** What the card is built with. */
export interface ProfilesDeps {
  /** The main process. */
  bridge: Bridge;
  /** The profiles and the switch between them. */
  persona: PersonaViewModel;
  /** Opens the account page. */
  openSettings: () => void;
}

/** Whether the card is up. */
interface State {
  /** Shown. */
  open: boolean;
}

/** The profile card. */
export class ProfilesViewModel extends ViewModel<State> {
  /** The profiles and the switch. */
  readonly persona: PersonaViewModel;
  /** The main process and Settings. */
  private readonly deps: ProfilesDeps;
  /** Shows and hides run in order, so a quick open and close never leave the page hidden. */
  private overlay = Promise.resolve();

  /** Closed until the circle is clicked. */
  constructor(deps: ProfilesDeps) {
    super({ open: false });
    this.deps = deps;
    this.persona = deps.persona;
  }

  /** The circle was clicked: opens the card, or closes it when it is up. */
  toggle(): void {
    if (this.state.open) return this.close();
    this.set({ open: true });
    this.sync();
    void this.persona.load();
  }

  /** Escape, a click outside, or a choice: the page comes back. */
  close(): void {
    if (!this.state.open) return;
    this.set({ open: false });
    this.sync();
  }

  /** Switches to profile `id` (nothing for the active one), and closes. */
  pick(id: string): void {
    if (id !== this.persona.state.active) this.persona.change(id);
    this.close();
  }

  /** Closes, then opens the account page. */
  settings(): void {
    this.close();
    this.deps.openSettings();
  }

  /** Shows or hides the overlay after any earlier one finished. */
  private sync(): void {
    const { bridge } = this.deps;
    const step = () => (this.state.open ? bridge.showOverlay(PROFILES_OVERLAY) : bridge.hideOverlay(PROFILES_OVERLAY));
    this.overlay = this.overlay.then(step, step).catch(() => {});
  }

  /** A disposed card never leaves the page hidden, and its profile list stops listening. */
  override dispose(): void {
    this.close();
    this.persona.dispose();
    super.dispose();
  }
}
