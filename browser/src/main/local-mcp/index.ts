/** Facade: Oya's MCP port on this computer, and the coding agents that can be connected to it. */
import type { AppServices } from '../app/services.ts';
import { LocalMcp } from './server.ts';

export { LocalMcp } from './server.ts';
export { AGENTS, installedAgents, type AgentStatus } from './agents.ts';

/** Whether this browser offers the port: a person's desktop, not a governed or cloud runtime, unless turned off. */
export function offersLocalMcp(deps: Pick<AppServices, 'governance'>, platform = process.platform): boolean {
  if (process.env.OYA_LOCAL_MCP === '0' || deps.governance.configuration) return false;
  return platform === 'darwin' || platform === 'win32';
}

/** Starts the port when this browser offers it; answers it either way so the footer can say. */
export async function startLocalMcp(deps: ConstructorParameters<typeof LocalMcp>[0] & Pick<AppServices, 'governance'>) {
  const mcp = new LocalMcp(deps);
  if (offersLocalMcp(deps)) await mcp.start();
  return mcp;
}
