/** Desktop builds run stock Electron: the tab preload asks for the persona source over this one channel. */
export const STOCK_PERSONA = {
  /** Synchronous, so the source runs before the document's own scripts. Empty on a native engine. */
  CHANNEL: 'oya:stock-persona-source',
  /** Only Oya's own tabs carry it: a synchronous call nobody answers would hang the renderer. */
  ARGUMENT: '--oya-stock-persona',
} as const;
