/**
 * The welcome panel's first-run checklist, drawn as a quiet stepper: one step
 * open at a time with its one action, the ones ahead dimmed, the ones done
 * folded to a tick and a line. There is only ever one primary button. The steps'
 * state lives in checklist-view-model.ts; this only draws it.
 */
import type { ReactNode } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import type { ChecklistState, ChecklistViewModel } from '../view-models/checklist-view-model.ts';
import type { ViewProps } from '../model/models.ts';
import { TEXT } from '../model/constants.ts';
import { Button } from '../../../ui/index.ts';

/** The steps by name, in order; DONE is past the last one. */
const STEP = { CHATGPT: 1, IMPORT: 2, DEFAULT: 3, DONE: 4 } as const;

/** Where a step stands. */
type StepState = 'done' | 'active' | 'ahead' | 'skipped';

/** One row's props. */
interface StepProps {
  /** Its number. */
  n: number;
  /** Where it stands. */
  state: StepState;
  /** What it is. */
  title: string;
  /** The line under the title. */
  detail: ReactNode;
  /** What shows while it is the open step. */
  children?: ReactNode;
}

/** The checklist's props: its view-model, and the two ways out of it. */
interface ChecklistProps extends ViewProps<ChecklistViewModel> {
  /** "Have an Oya account or API key?": the classic sign-in. */
  onAccount: () => void;
  /** "Skip for now": browse without connecting. */
  onSkip: () => void;
}

/** The open step: the first one not done (2 is skipped when the person kept their logins at home). */
function openStep({ progress: p, importLogins }: ChecklistState): number {
  if (!p?.signedIn || !p.chatgpt.connected) return STEP.CHATGPT;
  if (!p.imported && importLogins) return STEP.IMPORT;
  return p.isDefault ? STEP.DONE : STEP.DEFAULT;
}

/** A step's state from the open step's number. */
const stateOf = (n: number, open: number, skipped = false): StepState =>
  skipped ? 'skipped' : n < open ? 'done' : n === open ? 'active' : 'ahead';

/** One row of the stepper. */
function Step({ n, state, title, detail, children }: StepProps) {
  return (
    <li className="stepper-step" data-state={state} aria-current={state === 'active' ? 'step' : undefined}>
      <span className="stepper-marker" aria-hidden="true">
        {state === 'done' ? '✓' : state === 'skipped' ? '–' : n}
      </span>
      <div className="stepper-body">
        <strong>{title}</strong>
        <span className="stepper-detail">{detail}</span>
        {state === 'active' && children && <div className="stepper-action">{children}</div>}
      </div>
    </li>
  );
}

/** Step 1's action: the sign-in, and whether to bring the logins along. */
function ConnectAction({ vm }: ViewProps<ChecklistViewModel>) {
  const { connecting, importLogins } = useViewModel(vm);
  return (
    <>
      <button
        className="btn-connect setup-primary"
        id="btn-chatgpt"
        disabled={connecting}
        onClick={() => void vm.connectChatgpt()}
      >
        {connecting ? TEXT.chatgptWaiting : TEXT.chatgptConnect}
      </button>
      <label className="stepper-switch">
        <input type="checkbox" checked={importLogins} onChange={(e) => vm.toggleImport(e.target.checked)} />
        <span className="stepper-switch-track" aria-hidden="true" />
        {TEXT.chatgptImport}
      </label>
    </>
  );
}

/** The steps. */
function Steps({ vm }: ViewProps<ChecklistViewModel>) {
  const s = useViewModel(vm);
  const open = openStep(s);
  const skippedImport = open > STEP.CHATGPT && !s.progress?.imported && !s.importLogins;
  return (
    <ol className="stepper">
      <Step
        n={1}
        state={stateOf(1, open)}
        title={TEXT.stepChatgpt}
        detail={open > STEP.CHATGPT ? s.progress?.chatgpt.email || TEXT.stepChatgptDone : TEXT.stepChatgptHint}
      >
        <ConnectAction vm={vm} />
      </Step>
      <Step
        n={STEP.IMPORT}
        state={stateOf(STEP.IMPORT, open, skippedImport)}
        title={TEXT.stepImport}
        detail={
          skippedImport ? TEXT.stepImportSkipped : open === STEP.IMPORT ? TEXT.stepImportBusy : TEXT.stepImportHint
        }
      />
      <Step n={STEP.DEFAULT} state={stateOf(STEP.DEFAULT, open)} title={TEXT.stepDefault} detail={TEXT.stepDefaultHint}>
        <Button variant="secondary" type="button" onClick={() => void vm.makeDefault()}>
          {TEXT.makeDefault}
        </Button>
      </Step>
    </ol>
  );
}

/** The checklist view. */
export function ChecklistView({ vm, onAccount, onSkip }: ChecklistProps) {
  const s = useViewModel(vm);
  const ready = openStep(s) > STEP.CHATGPT;
  return (
    <div className="welcome-view stepper-view" data-view="checklist">
      <h2>{TEXT.checklistTitle}</h2>
      <p className="sub">{TEXT.checklistSub}</p>
      <Steps vm={vm} />
      <div role="alert" className="setup-error">
        {s.error}
      </div>
      {ready && (
        <button className="btn-connect setup-primary" id="btn-start" onClick={() => vm.finish()}>
          {TEXT.startBrowsing}
          <span className="welcome-arrow" aria-hidden="true">
            →
          </span>
        </button>
      )}
      <nav className="stepper-links" aria-label="Other ways in">
        <button type="button" id="btn-have-account" onClick={onAccount}>
          {TEXT.haveAccount}
        </button>
        <button type="button" onClick={onSkip}>
          {TEXT.skipForNow}
        </button>
      </nav>
    </div>
  );
}
