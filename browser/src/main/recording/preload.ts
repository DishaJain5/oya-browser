/** Browser-owned isolated recording bridge, installed before any tab or popup document can execute. */
import path from 'node:path';
import type { WebPreferences } from 'electron';
import { STOCK_PERSONA } from '../../shared/stock-persona.ts';
/**
 * Subframes run only the sandboxed preload; Node remains unavailable in every website world.
 * `stockPersona` (stock Electron, desktop) lets the preload ask the app for the persona source.
 */
export function recordingPreferences(appDir: string, stockPersona = false): WebPreferences {
  return {
    sandbox: true,
    contextIsolation: true,
    nodeIntegration: false,
    nodeIntegrationInSubFrames: true,
    preload: path.join(appDir, 'out', 'preload', 'recording.js'),
    ...(stockPersona ? { additionalArguments: [STOCK_PERSONA.ARGUMENT] } : {}),
  };
}
