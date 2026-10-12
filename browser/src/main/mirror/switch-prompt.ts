/**
 * After an import, the question of whether to browse with the profile it made.
 * Each imported browser is its own profile, with its own logins, so moving off
 * the one the person is using is their call, never a side effect of the import.
 */
import type { MessageBoxOptions } from 'electron';
import type { AppServices } from '../app/services.ts';
import { SWITCH_TO_IMPORTED } from './constants.ts';

/** The window to ask over, and the native dialog. */
type Deps = Pick<AppServices, 'electron' | 'shell'>;

/** The profile in use and the one the import made, by the names the profile menu shows. */
export interface SwitchNames {
  /** The profile Oya browses as now. */
  from: string;
  /** The profile the import made. */
  to: string;
}

/** The question; switching is offered first, staying is what Escape picks. */
function question({ from, to }: SwitchNames): MessageBoxOptions {
  return {
    type: 'question',
    message: `Browse with your ${to} logins now?`,
    detail: `Oya is browsing as ${from}, whose logins stay as they are. You can switch any time from the profile menu.`,
    buttons: [`Switch to ${to}`, `Stay on ${from}`],
    ...{ defaultId: SWITCH_TO_IMPORTED, cancelId: SWITCH_TO_IMPORTED + 1, noLink: true },
  };
}

/** Whether the person chose to move to the imported profile. */
export async function askToSwitch(deps: Deps, names: SwitchNames): Promise<boolean> {
  const { dialog } = deps.electron;
  const win = deps.shell.window;
  const options = question(names);
  const { response } = await (win ? dialog.showMessageBox(win, options) : dialog.showMessageBox(options));
  return response === SWITCH_TO_IMPORTED;
}
