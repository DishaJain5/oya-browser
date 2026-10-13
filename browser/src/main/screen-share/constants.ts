/** Native picker size, thumbnail bounds and bounded lifecycle checks. */
export const PICKER = {
  WIDTH: 780,
  HEIGHT: 620,
  THUMBNAIL: { width: 280, height: 158 },
  LOAD_TIMEOUT_MS: 10000,
  PREVIEW_TIMEOUT_MS: 3000,
  SOURCES_TIMEOUT_MS: 10000,
  OWNER_CHECK_MS: 200,
  TAB_LIMIT: 80,
  BACKGROUND: '#10120f',
  DESKTOP_ERROR:
    'Window and screen previews are unavailable. Check Oya’s Screen Recording permission in System Settings.',
} as const;

/** Browser-owned fixed-size sheet, styled consistently with native Oya dialogs. */
export const PICKER_WINDOW = {
  width: PICKER.WIDTH,
  height: PICKER.HEIGHT,
  show: false,
  title: 'Oya · Share a tab, window or screen',
  resizable: false,
  minimizable: false,
  maximizable: false,
  backgroundColor: PICKER.BACKGROUND,
};
/** Privilege isolation is independent of the requesting site's profile. */
export const PICKER_WEB = {
  sandbox: true,
  contextIsolation: true,
  nodeIntegration: false,
  webviewTag: false,
  disableDialogs: true,
};
/** Ask the OS for bounded thumbnails of every supported desktop source. */
export const DESKTOP_SOURCES = {
  types: ['window', 'screen'] as ('window' | 'screen')[],
  thumbnailSize: PICKER.THUMBNAIL,
};
