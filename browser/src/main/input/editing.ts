/** Native page editing and chords, shared by agent tools and workflow playback. */
import type { WebContents } from 'electron';
import type { Keyboard } from './keyboard.ts';
import { nativeChord } from './chords.ts';
import { KEY_DEFS as KEY_NAMES, Modifier } from './constants.ts';
/** Page editing exposes no clipboard or shell operation. */
type EditingView = { /** Exact native page. */ webContents: WebContents };
/** Native edits keep the browser's own selection, undo stack and focused-frame routing. */
export function editNativeText(view: EditingView, operation: unknown): void {
  const contents = view.webContents;
  if (contents.isDestroyed()) throw Error('View is destroyed');
  const edits = editingOperations(contents);
  if (typeof operation !== 'string' || !Object.hasOwn(edits, operation)) throw Error('Unsupported native text edit');
  edits[operation]();
}
/** Native select-all supplies platform editing defaults absent from raw macOS accelerators. */
export async function pressNativeChord(keyboard: Keyboard, view: EditingView, value: string): Promise<void> {
  const { key, modifiers } = nativeChord(value);
  const command = process.platform === 'darwin' ? Modifier.META : Modifier.CTRL;
  if (key.toLowerCase() === 'a' && modifiers === command) return editNativeText(view, 'select_all');
  if (modifiers && !Object.hasOwn(KEY_NAMES, key) && !/^[\x20-\x7e]$/.test(key))
    throw Error('Unsupported native page chord: composed text cannot carry modifiers');
  await keyboard.press(view, key, modifiers);
}

/** Explicit edit allowlist excludes arbitrary WebContents methods. */
function editingOperations(contents: WebContents): Record<string, () => void> {
  return { select_all: () => contents.selectAll(), undo: () => contents.undo(), redo: () => contents.redo() };
}
