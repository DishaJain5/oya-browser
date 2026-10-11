/**
 * The Ask pane (`#pane-chat`): the toolbar (profile picker, Model, Clear), the
 * Sign in and model cards, the conversation, and the input bar.
 */
import { ASK_TEXT } from '../model/constants.ts';
import { useViewModel } from '../../../hooks/index.ts';
import { Button, Icon, IconButton } from '../../../ui/index.ts';
import './ask.css';
import type { RendererServices } from '../../../app/services.ts';
import type { AskViewModel } from '../view-models/ask-view-model.ts';
import { personaLocked } from '../view-models/persona-view-model.ts';
import { ModelCard } from './model-card.tsx';
import { Messages } from './messages.tsx';
import { Composer } from './composer.tsx';

/** What the Ask pane is given. */
export interface AskPaneProps {
  /** Opens the saved playbook in the library. */
  onViewPlaybook?: (name: string) => void;
  /** The conversation and its collaborators. */
  ask: AskViewModel;
  /** The workspace panel: the pane in view, and whether it is open. */
  panel: RendererServices['panel'];
}

/** What the toolbar and the Sign in card are given. */
interface AskPartProps {
  /** The conversation and its collaborators. */
  ask: AskViewModel;
}

/** The pane's toolbar: the profile picker, the Model button and Clear. */
function AskToolbar({ ask }: AskPartProps) {
  const { sending } = useViewModel(ask);
  const persona = useViewModel(ask.persona);
  const { signedIn, status } = useViewModel(ask.model);
  return (
    <div className="studio-header chat-toolbar">
      <select
        id="chat-persona"
        aria-label="Profile"
        title="The profile Oya browses as. Switching reopens your tabs with that profile's cookies."
        disabled={personaLocked(persona.ready, sending)}
        value={persona.active}
        onChange={(e) => ask.persona.change(e.target.value)}
      >
        {persona.options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <Button
        id="chat-model-open"
        title={
          status.provider ? `${status.provider} · ${status.model}. Change model settings` : 'Change model settings'
        }
        hidden={!signedIn}
        onClick={() => void ask.model.openCard()}
      >
        {status.model || 'Model'}
      </Button>
      <IconButton id="chat-clear" label="Clear chat" data-icon="trash" onClick={() => ask.clear()} icon="trash" />
    </div>
  );
}

/** "Sign in to use Ask", while this browser has no project. */
function SignInCard({ ask }: AskPartProps) {
  const { signedIn } = useViewModel(ask.model);
  return (
    <div className="chat-card" id="chat-signin" hidden={signedIn}>
      <h2>Sign in to use Ask</h2>
      <p>Ask runs on your Oya project. Signing in takes one click in your web browser.</p>
      <Button type="button" variant="primary" id="chat-signin-button" onClick={() => ask.model.signIn()}>
        Sign in with Oya
      </Button>
    </div>
  );
}

/** Explicit continuation retains the conversation while starting another bounded run. */
function StepLimitCard({ ask }: AskPartProps) {
  const { limited, sending } = useViewModel(ask);
  if (!limited) return null;
  return (
    <div className="chat-card" role="status">
      <h2>This run reached its step limit</h2>
      <p>Continue from the current page with a fresh run budget. Your account’s usage allowance still applies.</p>
      <Button variant="primary" disabled={sending} onClick={() => void ask.continueRun()}>
        Continue task
      </Button>
      <Button disabled={sending} onClick={() => ask.clear()}>
        Start new task
      </Button>
    </div>
  );
}

/** Today's free steps are used up: what a free account adds, and the way to make one. */
function TrialCard({ ask }: AskPartProps) {
  const { trialSpent } = useViewModel(ask);
  if (!trialSpent) return null;
  return (
    <div className="chat-card trial-card" role="status">
      <span className="trial-card-mark" aria-hidden="true">
        <Icon name="spark" />
      </span>
      <h2>{ASK_TEXT.trialTitle}</h2>
      <p>{ASK_TEXT.trialBody}</p>
      <ul className="trial-card-perks">
        {ASK_TEXT.trialPerks.map((perk) => (
          <li key={perk}>{perk}</li>
        ))}
      </ul>
      <div className="chat-card-actions">
        <Button onClick={() => ask.dismissTrial()}>{ASK_TEXT.trialLater}</Button>
        <Button variant="primary" onClick={() => ask.createAccount()}>
          {ASK_TEXT.trialCreate}
        </Button>
      </div>
    </div>
  );
}

/** The Ask pane, shown while the panel is on Ask. */
export function AskPane({ ask, panel, onViewPlaybook }: AskPaneProps) {
  const { pane } = useViewModel(panel);
  return (
    <div className={pane === 'chat' ? 'dev-pane active' : 'dev-pane'} id="pane-chat">
      <AskToolbar ask={ask} />
      <SignInCard ask={ask} />
      <ModelCard model={ask.model} />
      <Messages ask={ask} onViewPlaybook={onViewPlaybook} />
      <StepLimitCard ask={ask} />
      <TrialCard ask={ask} />
      <Composer ask={ask} panel={panel} />
    </div>
  );
}
