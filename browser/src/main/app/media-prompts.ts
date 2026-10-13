/**
 * The words the device dialogs say: microphone and camera, screen share, and
 * the macOS denial. Kept apart from the rules so the copy reads as one family
 * and a test can pin it.
 */
import type { MessageBoxOptions } from 'electron';

/** What a site can ask for in the Allow dialog. */
export type DeviceKind = 'audio' | 'video';

/** The line every Allow dialog ends on: what Allow means, and where to undo it. */
const REMEMBERED = 'Allow is remembered for this site in this profile; forget it any time under History and bookmarks.';

/** Block first, so Enter and Escape both refuse. */
const BLOCK_OR_ALLOW: Pick<MessageBoxOptions, 'buttons' | 'defaultId' | 'cancelId'> = {
  buttons: ['Block', 'Allow'],
  defaultId: 0,
  cancelId: 0,
};

/** Tells the person exactly which devices the website requested. */
export function allowDevicesPrompt(origin: string, types: DeviceKind[]): MessageBoxOptions {
  const devices = types.map((type) => (type === 'audio' ? 'microphone' : 'camera')).join(' and ');
  return {
    message: `Allow ${devices}?`,
    detail: `${origin} wants to use your ${devices}. Allow only if you trust this site. ${REMEMBERED}`,
    ...BLOCK_OR_ALLOW,
  };
}

/** Site consent permits requesting a share; the person still chooses a source for every stream. */
export function shareScreenPrompt(origin: string): MessageBoxOptions {
  return {
    message: `Share your screen with ${origin}?`,
    detail: `You will choose an Oya tab, a window or an entire screen before sharing starts. You choose again for every new share. ${REMEMBERED}`,
    ...BLOCK_OR_ALLOW,
  };
}

/** Explains an OS denial rather than leaving a call silently without a device. */
export const OS_DENIED_PROMPT: MessageBoxOptions = {
  message: 'Device access is blocked in macOS',
  detail:
    'Allow Oya Browser in System Settings → Privacy & Security → Microphone, Camera or Screen & System Audio Recording, then rejoin the call.',
  buttons: ['OK'],
};

/** Screen Recording has its own pane and may require adding the installed application manually. */
export const SCREEN_DENIED_PROMPT: MessageBoxOptions = {
  message: 'Allow Oya to share your screen',
  detail:
    'In System Settings → Privacy & Security → Screen & System Audio Recording, enable Oya Browser. If it is missing, click + and select Oya Browser in Applications. Then quit and reopen Oya and try sharing again.',
  buttons: ['Not now', 'Open System Settings'],
  defaultId: 1,
  cancelId: 0,
};
