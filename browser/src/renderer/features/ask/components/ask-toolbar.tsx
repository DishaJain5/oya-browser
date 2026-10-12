/**
 * The Ask pane's controls: New chat, named rather than a bare bin, at the top;
 * and the profile and model dropdowns the composer shows under the Ask box.
 * Styles are in ask-toolbar.css.
 */
import { useViewModel } from '../../../hooks/index.ts';
import { Icon } from '../../../ui/index.ts';
import type { AskViewModel } from '../view-models/ask-view-model.ts';
import { personaLocked } from '../view-models/persona-view-model.ts';
import './ask-toolbar.css';

/** What the toolbar's parts are given. */
export interface ToolbarProps {
  /** The shown tab's conversation. */
  ask: AskViewModel;
}

/** The profile Oya browses as: the panel's own dropdown, without its box. */
export function ProfileChip({ ask }: ToolbarProps) {
  const { sending } = useViewModel(ask);
  const persona = useViewModel(ask.persona);
  return (
    <select
      id="chat-persona"
      className="ask-profile"
      aria-label="Profile"
      title="The profile Oya browses as. Switching reopens your tabs with its cookies."
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
  );
}

/** The model this chat runs on, as a switch: it opens the model settings. */
export function ModelChip({ ask }: ToolbarProps) {
  const { signedIn, status } = useViewModel(ask.model);
  if (!signedIn) return null;
  const model = status.model || 'Choose a model';
  return (
    <button
      type="button"
      id="chat-model-open"
      className="ask-chip ask-model"
      aria-label={`Model: ${model}. Switch model`}
      title={status.provider ? `${status.provider} · ${model}. Switch model` : 'Switch model'}
      onClick={() => void ask.model.openCard()}
    >
      <span className="ask-chip-text">{model}</span>
      <span className="ask-chip-chevron" aria-hidden="true">
        <Icon name="chevron" />
      </span>
    </button>
  );
}

/** Starts this tab's chat over; named, so it is not taken for deleting anything else. */
function NewChat({ ask }: ToolbarProps) {
  const { items, sending } = useViewModel(ask);
  return (
    <button
      type="button"
      id="chat-clear"
      className="ask-new"
      aria-label="New chat"
      title="Start a new chat in this tab"
      disabled={!items.length && !sending}
      onClick={() => ask.clear()}
    >
      <Icon name="plus" />
      <span>New chat</span>
    </button>
  );
}

/** The toolbar. */
export function AskToolbar({ ask }: ToolbarProps) {
  return (
    <div className="ask-toolbar">
      <NewChat ask={ask} />
    </div>
  );
}
