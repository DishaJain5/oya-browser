/**
 * What a routine does: ask the agent a prompt, or play one of the project's
 * saved playbooks. The prompt box hides while a playbook is chosen.
 */
'use client';

import FieldHelp from './field-help';
import { usePlaybookNames } from './use-playbook-names';
import { EDITOR_LIMITS, EDITOR_TEXT } from './constants';
import type { RoutineForm } from './use-routine-form';

/** Props for the task fields. */
interface Props {
  /** The caller's API key, for the playbook list. */
  apiKey: string;
  /** The form. */
  form: RoutineForm;
}

/** The Does choice and, when it asks a prompt, the prompt. */
export default function TaskField({ apiKey, form }: Props) {
  const { names, error } = usePlaybookNames(apiKey);
  const { fields, set, errors } = form;
  const options = fields.playbook && !names.includes(fields.playbook) ? [fields.playbook, ...names] : names;
  return (
    <>
      <label className="label" htmlFor="rt-task">
        {EDITOR_TEXT.task}
      </label>
      <select
        id="rt-task"
        className="field"
        value={fields.playbook}
        onChange={(e) => set({ playbook: e.target.value })}
        aria-describedby="rt-task-help"
      >
        <option value="">{EDITOR_TEXT.askPrompt}</option>
        {options.length > 0 && (
          <optgroup label={EDITOR_TEXT.playGroup}>
            {options.map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </optgroup>
        )}
      </select>
      <FieldHelp id="rt-task-help" error={error ? EDITOR_TEXT.playbooksError(error) : undefined} />
      {!fields.playbook && (
        <div className="mt-3">
          <label className="label" htmlFor="rt-prompt">
            {EDITOR_TEXT.prompt}
          </label>
          <textarea
            id="rt-prompt"
            className="field"
            rows={EDITOR_LIMITS.promptRows}
            maxLength={EDITOR_LIMITS.promptChars}
            placeholder={EDITOR_TEXT.promptPlaceholder}
            value={fields.prompt}
            onChange={(e) => set({ prompt: e.target.value })}
            aria-describedby="rt-prompt-help"
            aria-invalid={Boolean(errors.prompt)}
          />
          <FieldHelp id="rt-prompt-help" error={errors.prompt} />
        </div>
      )}
    </>
  );
}
