/**
 * The toolbar's connection pieces: the update pill and the connection pill
 * (index.html's #update-pill and #conn-pill), and the version line in the
 * shell dialog's footer (#app-version).
 */
import { useViewModel } from '../../../hooks/index.ts';
import type { UpdatesState, UpdatesViewModel } from '../view-models/updates-view-model.ts';
import { TEXT } from '../model/constants.ts';
import { Icon } from '../../../ui/index.ts';
import type { ConnectionPillViewModel } from '../view-models/connection-pill-view-model.ts';
import type { ViewProps } from '../model/models.ts';
import { accountCard, type AccountViewModel } from '../view-models/account-view-model.ts';
import './toolbar-pills.css';

/** The update pill: shown while there is something to say; checks, or restarts into a staged update. */
export function UpdatePill({ vm }: ViewProps<UpdatesViewModel>) {
  const s = useViewModel(vm);
  return (
    <button
      className={s.attention ? 'update-pill attention' : 'update-pill'}
      id="update-pill"
      hidden={s.hidden}
      disabled={s.disabled}
      title={s.title}
      onClick={() => void vm.click()}
    >
      {s.text}
    </button>
  );
}

/** The footer button's words: "Check for updates" until there is news or a check to report. */
const checkLabel = (s: UpdatesState): string => (s.checked || !s.hidden || s.disabled ? s.text : TEXT.checkUpdates);

/** The version line in the shell dialog's footer, with a button that checks for updates (or restarts into one). */
export function AppVersion({ vm }: ViewProps<UpdatesViewModel>) {
  const s = useViewModel(vm);
  return (
    <span id="app-version">
      {s.version}
      <button className="check-updates" disabled={s.disabled} title={s.title} onClick={() => void vm.click()}>
        <Icon name="reload" />
        {checkLabel(s)}
      </button>
    </span>
  );
}

/** What the profile button is given. */
export interface ProfileButtonProps {
  /** The connection: connected, connecting or offline. */
  pill: ConnectionPillViewModel;
  /** Who is signed in, for the avatar's initial. */
  account: AccountViewModel;
  /** Opens the profile card. */
  onOpen: () => void;
}

/**
 * The profile button at the toolbar's end, as in Chrome: the account's initial, with
 * a dot that says whether this browser is online (green), connecting (amber) or
 * offline (grey). It opens the profile menu, as Chrome's does: the browsing profiles with
 * a tick on the active one, and Settings. The words stay for assistive tech and tooltips.
 */
export function ProfileButton({ pill, account, onOpen }: ProfileButtonProps) {
  const s = useViewModel(pill);
  const { initial } = accountCard(useViewModel(account));
  return (
    <button
      className={`${s.className} profile-button`}
      id="conn-pill"
      title={s.title}
      aria-label={`Profiles and settings, ${s.label}`}
      aria-haspopup="dialog"
      onClick={onOpen}
    >
      <span className="profile-avatar" aria-hidden="true">
        {initial}
      </span>
      <span className="dot" aria-hidden="true"></span>
      <span id="conn-label" className="sr-only">
        {s.label}
      </span>
    </button>
  );
}
