/** Desktop builds run stock Electron: the tab preload asks for the persona source over this one channel. */
export const STOCK_PERSONA = {
  /** Synchronous, so the source runs before the document's own scripts. Empty on a native engine. */
  CHANNEL: 'oya:stock-persona-source',
  /** Only Oya's own tabs carry it: a synchronous call nobody answers would hang the renderer. */
  ARGUMENT: '--oya-stock-persona',
  /** Main asks a tab's preload to run code in an isolated world (stock executeJavaScript waits for the page to finish loading). */
  RUN: 'oya:stock-world-run',
  /** The preload received a run request, before running anything: no reply means it never ran. */
  ACK: 'oya:stock-world-ack',
  /** The run's value or error message. */
  RESULT: 'oya:stock-world-result',
} as const;
