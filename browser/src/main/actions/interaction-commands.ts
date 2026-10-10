/** Native page interactions available equally to agents and desktop workflows. */
import { editNativeText, pressNativeChord, nativeWheel } from '../input/index.ts';
import type { PageHandler } from './page-commands.ts';
/** Each operation enters through the normal command ownership gate. */
export const INTERACTION_COMMANDS: Readonly<Record<string, PageHandler>> = {
  async key_chord(driver, id, params, view) {
    await pressNativeChord(driver.keyboard, view, params?.key ?? '');
    driver.deps.sendResult(id, true, { key: params?.key });
  },
  edit_text(driver, id, params, view) {
    editNativeText(view, params?.operation);
    driver.deps.sendResult(id, true, { operation: params?.operation });
  },
  scroll_at(driver, id, params, view) {
    nativeWheel(view, params?.x ?? NaN, params?.y ?? NaN, params?.delta_x ?? NaN, params?.delta_y ?? NaN);
    driver.deps.sendResult(id, true, { scrolled: true });
  },
};
