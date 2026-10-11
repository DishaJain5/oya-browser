/**
 * The toolbar's connection pieces: the update pill and the connection pill
 * (index.html's #update-pill and #conn-pill), and the version line in the
 * shell dialog's footer (#app-version).
 */
import { useViewModel } from '../../../hooks/index.ts';
import type { UpdatesViewModel } from '../view-models/updates-view-model.ts';
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

/** The version line in the shell dialog's footer. */
export function AppVersion({ vm }: ViewProps<UpdatesViewModel>) {
  return <span id="app-version">{useViewModel(vm).version}</span>;
}

/** What the profile button is given. */
export interface ProfileButtonProps {
  /** The connection: connected, connecting or offline. */
  pill: ConnectionPillViewModel;
  /** Who is signed in, for the avatar's initial. */
  account: AccountViewModel;
}

/**
 * The profile button at the toolbar's end, as in Chrome: the account's initial, with
 * a dot that says whether this browser is online (green), connecting (amber) or
 * offline (grey). It opens Settings. The words stay for assistive tech and tooltips.
 */
export function ProfileButton({ pill, account }: ProfileButtonProps) {
  const s = useViewModel(pill);
  const { initial } = accountCard(useViewModel(account));
  return (
    <button
      className={`${s.className} profile-button`}
      id="conn-pill"
      title={s.title}
      aria-label={`Settings, ${s.label}`}
      onClick={() => pill.click()}
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
