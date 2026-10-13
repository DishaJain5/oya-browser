/** Compact navigation for the settings people revisit, using the shared Oya type and icon system. */
import { Icon, type IconName } from '../../../ui/index.ts';
import { useViewModel } from '../../../hooks/index.ts';
import { accountCard, type AccountViewModel } from '../view-models/account-view-model.ts';
import type { SettingsPage } from '../view-models/shell-dialog-view-model.ts';
/** Navigation label, recognizable icon and a brief purpose for each section. */
const SECTIONS: Record<SettingsPage, [string, IconName, string]> = {
  general: ['General', 'settings', 'Appearance and how this browser works.'],
  logins: ['Logins & sync', 'key', 'Bring your saved logins with you.'],
  account: ['Account', 'profile', 'Your identity and connection to Oya.'],
};
/** Navigation buttons keep ordinary keyboard focus and expose their selected destination. */
export function SettingsNavigation({
  page,
  choose,
  account,
}: {
  /** The visible settings section. */
  page: SettingsPage;
  /** Select a section without closing Settings. */
  choose: (page: SettingsPage) => void;
  /** Account summary, using current connection state. */
  account: AccountViewModel;
}) {
  const card = accountCard(useViewModel(account));
  return (
    <aside className="settings-sidebar">
      <nav aria-label="Settings sections">
        {(Object.keys(SECTIONS) as SettingsPage[]).map((id) => (
          <button
            key={id}
            id={`settings-nav-${id}`}
            aria-current={page === id ? 'page' : undefined}
            aria-controls={`settings-${id}`}
            onClick={() => choose(id)}
          >
            <Icon name={SECTIONS[id][1]} />
            <span>{SECTIONS[id][0]}</span>
          </button>
        ))}
      </nav>
      <div className="settings-identity" title={card.connectionText}>
        <span className="settings-mini-avatar" aria-hidden="true">
          {card.initial}
        </span>
        <div>
          <strong>{card.connection === 'connected' && card.project ? card.project : 'Oya Browser'}</strong>
          <span>
            {card.connection === 'connected'
              ? 'Connected'
              : card.connection === 'reconnecting'
                ? 'Reconnecting…'
                : 'Offline'}
          </span>
        </div>
      </div>
    </aside>
  );
}
/** One clear heading describes the visible group without repeating every row label. */
export function SettingsHeading({
  page,
}: {
  /** Selected section determines title and description. */
  page: SettingsPage;
}) {
  return (
    <div className="settings-heading">
      <h3>{SECTIONS[page][0]}</h3>
      <p>{SECTIONS[page][2]}</p>
    </div>
  );
}
