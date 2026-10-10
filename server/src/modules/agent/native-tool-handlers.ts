/** Native tools always use the standard authenticated, capability-checked command path. */
import { sendCommand } from '../browsers/socket.ts';
import { NATIVE_TOOL_NAMES } from './native-tools.ts';
/** Keep structured diagnostic metadata intact for the agent and MCP. */
async function run(browserId: string, name: string, args: Record<string, unknown>) {
  const result = await sendCommand(browserId, name, args);
  return result.ok ? JSON.stringify(result.data ?? {}) : `Error: ${result.error}`;
}
/** Shared agent/MCP dispatch; browser validation remains authoritative. */
export const NATIVE_TOOL_HANDLERS = Object.fromEntries(
  NATIVE_TOOL_NAMES.map((name) => [
    name,
    (browserId: string, args: Record<string, unknown>) => run(browserId, name, args),
  ]),
);
