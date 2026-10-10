/** Shared native page key parsing for agent commands and workflow playback. */
import { KEY_DEFS, Modifier } from './constants.ts';
/** Modifier spellings accepted by exported workflows. */
const MODIFIERS: Record<string, number> = {
  Control: Modifier.CTRL,
  Ctrl: Modifier.CTRL,
  Meta: Modifier.META,
  Shift: Modifier.SHIFT,
  Alt: Modifier.ALT,
};
/** Keys that alter native browser UI have no page-input meaning. */
const BLOCKED = new Set(['F11', 'F12', 'F5', 'BrowserBack', 'BrowserForward', 'ZoomIn', 'ZoomOut']);
/** Parsed physical key and held modifier flags. */
export interface NativeChord {
  /** Logical key. */ key: string;
  /** Native input modifier bits. */ modifiers: number;
}
/** Reject malformed keys before any draft action; literal plus is a valid printable key. */
export function nativeChord(value: string): NativeChord {
  if (typeof value !== 'string') throw Error('Native key chord must be a string');
  const parts = value === '+' ? ['+'] : value.split('+');
  const key = parts.pop() || '';
  if (BLOCKED.has(key) || (!Object.hasOwn(KEY_DEFS, key) && [...key].length !== 1))
    throw Error('Unsupported native page key: ' + value);
  return { key, modifiers: parts.reduce((bits, part) => bits | modifier(part), 0) };
}
/** Platform-dependent shortcut semantics stay centralized in one parser. */
function modifier(name: string): number {
  if (name === 'ControlOrMeta') return process.platform === 'darwin' ? Modifier.META : Modifier.CTRL;
  if (!Object.hasOwn(MODIFIERS, name)) throw Error('Unsupported native page modifier: ' + name);
  return MODIFIERS[name];
}
