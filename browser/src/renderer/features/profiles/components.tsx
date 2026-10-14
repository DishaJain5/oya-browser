/**
 * The profile card, drawn like Chrome's: the signed-in person at the top, the
 * browsing profiles as rows with a coloured initial and a tick on the active one,
 * and Settings at the foot. It hangs under the profile circle at the toolbar's end.
 */
import { useViewModel } from '../../hooks/index.ts';
import { Dialog, Icon } from '../../ui/index.ts';
import type { PersonaOption } from '../ask/index.ts';
import { accountCard, type AccountCard, type AccountViewModel } from '../connection/index.ts';
import type { ProfilesViewModel } from './view-model.ts';
import { AVATAR_TONES, PROFILES_TEXT as TEXT } from './constants.ts';
import './profiles.css';

/** The card's props: its view-model and the signed-in person. */
interface Props {
  /** The card. */
  vm: ProfilesViewModel;
  /** Who is signed in. */
  account: AccountViewModel;
}

/** The same name always gets the same colour. */
function toneOf(name: string): number {
  let sum = 0;
  for (const c of name) sum += c.charCodeAt(0);
  return sum % AVATAR_TONES;
}

/** The signed-in person: initial, name, email. */
function Who({ card }: { /** The signed-in person, in words. */ card: AccountCard }) {
  return (
    <header className="profiles-who">
      <span className="profiles-avatar profiles-avatar-lg">{card.initial}</span>
      <strong>{card.title}</strong>
      {card.email && <span>{card.email}</span>}
    </header>
  );
}

/** One profile: its initial, its name, and a tick when it is the one in use. */
function Row({
  vm,
  option,
  active,
  ready,
}: {
  /** The card. */ vm: ProfilesViewModel;
  /** The profile. */ option: PersonaOption;
  /** In use. */ active: boolean;
  /** Switchable. */ ready: boolean;
}) {
  return (
    <li>
      <button
        type="button"
        className="profiles-row"
        id={active ? 'profiles-active' : undefined}
        aria-current={active}
        disabled={!ready && !active}
        onClick={() => vm.pick(option.value)}
      >
        <span className="profiles-avatar" data-tone={toneOf(option.label)}>
          {option.label[0]?.toUpperCase()}
        </span>
        <span className="profiles-name">{option.label}</span>
        {active && <Icon name="check" />}
      </button>
    </li>
  );
}

/** The browsing profiles, or why they cannot be switched yet. */
function Profiles({ vm }: Pick<Props, 'vm'>) {
  const { options, active, ready } = useViewModel(vm.persona);
  return (
    <section className="profiles-list" aria-label={TEXT.heading}>
      <h3>{TEXT.heading}</h3>
      <ul>
        {options.map((option) => (
          <Row key={option.value} vm={vm} option={option} active={option.value === active} ready={ready} />
        ))}
      </ul>
      {!ready && <p className="profiles-note">{TEXT.offline}</p>}
    </section>
  );
}

/** The profile card. */
export function ProfilesCard({ vm, account }: Props) {
  const { open } = useViewModel(vm);
  const card = accountCard(useViewModel(account));
  return (
    <Dialog
      open={open}
      hidden={!open}
      onClose={() => vm.close()}
      focusId="profiles-active"
      id="profiles-overlay"
      className="profiles-overlay"
      label={TEXT.label}
    >
      <div className="profiles-card">
        <Who card={card} />
        <Profiles vm={vm} />
        <button
          type="button"
          id="profiles-settings"
          className="profiles-row profiles-settings"
          onClick={() => vm.settings()}
        >
          <Icon name="settings" />
          <span className="profiles-name">{TEXT.settings}</span>
        </button>
      </div>
    </Dialog>
  );
}
