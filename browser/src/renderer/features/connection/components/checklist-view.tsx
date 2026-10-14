/**
 * The welcome panel's first-run checklist, drawn as a quiet stepper: one step
 * open at a time with its one action, the ones ahead dimmed, the ones done
 * folded to a tick and a line. There is only ever one primary button. The steps'
 * state lives in checklist-view-model.ts; this only draws it.
 */
import type { ReactNode } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import type { ChecklistState, ChecklistViewModel, ConnectWay } from '../view-models/checklist-view-model.ts';
import type { ViewProps } from '../model/models.ts';
import { TEXT } from '../model/constants.ts';
import { WAY_ICONS } from './way-icons.tsx';
import { Button, TextField } from '../../../ui/index.ts';

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

/** The ways out of the checklist: the Oya sign-ins it hands to the setup screen, and skipping. */
interface Exits {
  /** "Sign in with Oya": the console sign-in in the web browser. */
  onSignIn: () => void;
  /** "Enter server and key": the manual form. */
  onManual: () => void;
}

/** The checklist's props: its view-model and the ways out of it. */
interface ChecklistProps extends ViewProps<ChecklistViewModel>, Exits {
  /** "Skip for now": browse without connecting. */
  onSkip: () => void;
}

/** Props for the parts of step 1 that can leave the checklist. */
type ExitProps = ViewProps<ChecklistViewModel> & Exits;

/** The open step: the first one not done (2 is skipped when the person kept their logins at home). */
function openStep({ progress: p, importLogins }: ChecklistState): number {
  if (!p?.signedIn || !p.hasModel) return STEP.CHATGPT;
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

/** "Bring in my logins", shown under either way to connect. */
function ImportSwitch({ vm }: ViewProps<ChecklistViewModel>) {
  const { importLogins } = useViewModel(vm);
  return (
    <label className="stepper-switch">
      <input type="checkbox" checked={importLogins} onChange={(e) => vm.toggleImport(e.target.checked)} />
      <span className="stepper-switch-track" aria-hidden="true" />
      {TEXT.chatgptImport}
    </label>
  );
}

/** Step 1's tabs, in order: each way in, its label and its id. */
const WAYS: [ConnectWay, string, string][] = [
  ['oya', TEXT.wayOya, 'tab-oya'],
  ['gemini', TEXT.wayGemini, 'tab-gemini'],
  ['chatgpt', TEXT.wayChatgpt, 'tab-chatgpt'],
  ['key', TEXT.wayKey, 'tab-key'],
];

/** Step 1's switch between the ways in, drawn as one quiet segmented control so none shouts. */
function WaySwitch({ vm }: ViewProps<ChecklistViewModel>) {
  const { way, connecting } = useViewModel(vm);
  return (
    <div className="stepper-segment" role="group" aria-label={TEXT.stepChatgpt}>
      {WAYS.map(([id, label, domId]) => (
        <button
          key={id}
          id={domId}
          type="button"
          aria-pressed={way === id}
          disabled={connecting}
          onClick={() => vm.pick(id)}
        >
          {WAY_ICONS[id]}
          {label}
        </button>
      ))}
    </div>
  );
}

/** A way out's button: its id, words, the line under it, and what it does. */
interface ExitActionProps {
  /** The button's id. */
  id: string;
  /** Its words. */
  label: string;
  /** The line under it. */
  hint: string;
  /** What clicking it does. */
  onClick: () => void;
}

/** One way that leaves the checklist for the setup screen: its button and a line under it. */
function ExitAction({ id, label, hint, onClick }: ExitActionProps) {
  return (
    <>
      <button className="btn-connect setup-primary" id={id} onClick={onClick}>
        {label}
        <span className="welcome-arrow" aria-hidden="true">
          →
        </span>
      </button>
      <span className="stepper-detail">{hint}</span>
    </>
  );
}

/** The ChatGPT way: one button, the sign-in finishes in the web browser. */
function ChatgptButton({ vm }: ViewProps<ChecklistViewModel>) {
  const { connecting } = useViewModel(vm);
  return (
    <button
      className="btn-connect setup-primary"
      id="btn-chatgpt"
      disabled={connecting}
      onClick={() => void vm.connectChatgpt()}
    >
      {connecting ? TEXT.chatgptWaiting : TEXT.chatgptConnect}
    </button>
  );
}

/** The Gemini key form: an email for the Oya key and a free key for the model. */
function GeminiForm({ vm }: ViewProps<ChecklistViewModel>) {
  const { connecting, email, key } = useViewModel(vm);
  const getKey = (
    <button type="button" className="stepper-inline-link" onClick={() => vm.getKey()}>
      {TEXT.geminiGetKey}
    </button>
  );
  return (
    <form
      className="stepper-form"
      onSubmit={(e) => {
        e.preventDefault();
        void vm.connectGemini();
      }}
    >
      <TextField
        id="gemini-email"
        label={TEXT.geminiEmail}
        type="email"
        autoComplete="email"
        value={email}
        onChange={(e) => vm.type({ email: e.target.value })}
      />
      <TextField
        id="gemini-key"
        label={TEXT.geminiKey}
        type="password"
        placeholder={TEXT.geminiKeyHint}
        value={key}
        hint={getKey}
        onChange={(e) => vm.type({ key: e.target.value })}
      />
      <button className="btn-connect setup-primary" id="btn-gemini" type="submit" disabled={connecting}>
        {connecting ? TEXT.geminiWaiting : TEXT.geminiConnect}
      </button>
    </form>
  );
}

/** Step 1's action: the switch, then the picked way's button or form (logins come along only on ChatGPT and Gemini). */
function ConnectAction({ vm, onSignIn, onManual }: ExitProps) {
  const { way } = useViewModel(vm);
  return (
    <>
      <WaySwitch vm={vm} />
      {way === 'chatgpt' && <ChatgptButton vm={vm} />}
      {way === 'gemini' && <GeminiForm vm={vm} />}
      {way === 'oya' && <ExitAction id="btn-signin" label={TEXT.oyaSignIn} hint={TEXT.oyaHint} onClick={onSignIn} />}
      {way === 'key' && <ExitAction id="btn-manual" label={TEXT.keyConnect} hint={TEXT.keyHint} onClick={onManual} />}
      {(way === 'chatgpt' || way === 'gemini') && <ImportSwitch vm={vm} />}
    </>
  );
}

/** The steps. */
function Steps({ vm, onSignIn, onManual }: ExitProps) {
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
        <ConnectAction vm={vm} onSignIn={onSignIn} onManual={onManual} />
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
export function ChecklistView({ vm, onSignIn, onManual, onSkip }: ChecklistProps) {
  const s = useViewModel(vm);
  const ready = openStep(s) > STEP.CHATGPT;
  return (
    <div className="welcome-view stepper-view" data-view="checklist">
      <h2>{TEXT.checklistTitle}</h2>
      <p className="sub">{TEXT.checklistSub}</p>
      <Steps vm={vm} onSignIn={onSignIn} onManual={onManual} />
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
        <button type="button" onClick={onSkip}>
          {TEXT.skipForNow}
        </button>
      </nav>
    </div>
  );
}
