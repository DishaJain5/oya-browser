/**
 * The footer's coding-agent calls: which agents this computer has, and adding
 * Oya's MCP port to one or taking it out. Only the person's click does either.
 */
import type { AppServices } from '../app/services.ts';
import type { HandlersOf } from './handle.ts';
import { AGENTS, installedAgents } from '../local-mcp/index.ts';
import type { LocalAgent } from '../../shared/ipc.ts';

/** The services these calls use. */
type Deps = Pick<AppServices, 'localMcp'>;

/** The channels this group answers. */
type Channel = 'local-agents' | 'connect-agent' | 'disconnect-agent';

/** The coding-agent handlers. */
export class AgentHandlers {
  /** Channel → handler. */
  readonly handlers: HandlersOf<Channel> = {
    'local-agents': () => this.list(),
    'connect-agent': (_e, id) => this.change(String(id), true),
    'disconnect-agent': (_e, id) => this.change(String(id), false),
  };
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process services. */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** The installed agents, or none when this browser offers no port. */
  private list(): LocalAgent[] {
    return this.deps.localMcp?.token ? installedAgents() : [];
  }

  /** Adds Oya to agent `id` or takes it out; answers the list as it now stands. */
  private async change(id: string, connect: boolean): Promise<LocalAgent[]> {
    const mcp = this.deps.localMcp;
    if (!Object.hasOwn(AGENTS, id) || !mcp?.token) throw new Error('That agent cannot be connected here.');
    if (connect) await AGENTS[id].connect({ url: mcp.url, token: mcp.token });
    else await AGENTS[id].disconnect();
    return this.list();
  }
}
