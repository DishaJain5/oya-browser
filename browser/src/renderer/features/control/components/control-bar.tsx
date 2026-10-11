/**
 * The toolbar's OyaAI Mode / Human Mode switch (index.html's #control-status):
 * who drives, and choosing the other side hands control over (or resumes the
 * agent after a pause). It also installs
 * the watch-only guard and gate over the toolbar's page actions, which other
 * features render (see use-control-guard.ts).
 */
import { useViewModel } from '../../../hooks/index.ts';
import type { ReactNode } from 'react';
import { controlLook, modeSwitch, type ControlViewModel, type SwitchSide } from '../view-models/control-view-model.ts';
import { ACTION_ID, MODES } from '../model/constants.ts';
import { useControlGate, useControlGuard } from '../hooks/use-control-guard.ts';
import './control-bar.css';

/** What the control bar is given. */
export interface ControlBarProps {
  /** The control ViewModel. */
  vm: ControlViewModel;
}

/** The OyaAI Mode / Human Mode switch, with a status beside it while control changes hands or cannot. */
export function ControlBar({ vm }: ControlBarProps) {
  const state = useViewModel(vm);
  useControlGuard(vm);
  useControlGate(state);
  const look = controlLook(state);
  const modes = modeSwitch(state);
  const toAgent = () => void (look.mode === 'paused' ? vm.resume() : vm.toggle());
  return (
    <div className="control-status" id="control-status" data-mode={look.mode} title={look.title}>
      <span id="control-label" role="status" hidden={!modes.status}>
        {modes.status}
      </span>
      <div className="control-switch" role="group" aria-label="Who drives this browser" hidden={modes.hidden}>
        <Side id="control-agent" side={modes.agent} onChoose={toAgent}>
          <span className="control-orbit" aria-hidden="true"></span>
          {MODES.agent}
        </Side>
        <Side id={ACTION_ID} side={modes.human} onChoose={() => void vm.toggle()}>
          {MODES.human}
        </Side>
      </div>
    </div>
  );
}

/** What one side of the switch is given. */
interface SideProps {
  /** Its element id. */
  id: string;
  /** Whether it is in effect, can be chosen, and what choosing it does. */
  side: SwitchSide;
  /** Choose it. */
  onChoose: () => void;
  /** Its label. */
  children: ReactNode;
}

/** One side of the switch: pressed while in effect. */
function Side({ id, side, onChoose, children }: SideProps) {
  return (
    <button id={id} aria-pressed={side.on} disabled={side.disabled} title={side.title} onClick={onChoose}>
      {children}
    </button>
  );
}
