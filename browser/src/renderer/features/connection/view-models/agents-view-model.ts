/**
 * The footer's coding agents: Claude Code, Codex and the others this computer
 * has, whether each lists Oya's MCP port, and connecting or disconnecting one.
 */
import type { LocalAgent } from '../../../../shared/ipc.ts';
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import { messageOf } from '../model/models.ts';

/** What the footer shows. */
export interface AgentsState {
  /** The installed agents. */
  agents: LocalAgent[];
  /** The agent being changed ('' for none). */
  busy: string;
  /** What the last change did, or why it failed ('' for nothing to say). */
  note: string;
}

/** The parts of the bridge the agents use. */
export type AgentsBridge = Pick<OyaBrowser, 'localAgents' | 'connectAgent' | 'disconnectAgent'>;

/** Agents whose config Oya can tell to run its tools without asking; the others ask the first time. */
const ALLOWED_ON_CONNECT = ['claude-code', 'codex'];

/** A list answer, or none when it is not a list. */
const listOf = (answer: unknown): LocalAgent[] => (Array.isArray(answer) ? (answer as LocalAgent[]) : []);

/** The footer's coding agents. */
export class AgentsViewModel extends ViewModel<AgentsState> {
  /** The main process. */
  private readonly bridge: AgentsBridge;

  /** Reads which agents this computer has. */
  constructor(bridge: AgentsBridge) {
    super({ agents: [], busy: '', note: '' });
    this.bridge = bridge;
    void this.refresh();
  }

  /** Reads the agents again, after one was installed or changed elsewhere. */
  async refresh(): Promise<void> {
    this.set({ agents: listOf(await this.bridge.localAgents().catch(() => [])) });
  }

  /** Connects agent `id` when it is not, disconnects it when it is. */
  async toggle(id: string): Promise<void> {
    const agent = this.state.agents.find((a) => a.id === id);
    if (!agent || this.state.busy) return;
    this.set({ busy: id, note: '' });
    await this.change(agent);
    this.set({ busy: '' });
  }

  /** Adds Oya to `agent` or takes it out, and says how that went. */
  private change(agent: LocalAgent): Promise<void> {
    const { id, connected } = agent;
    return (connected ? this.bridge.disconnectAgent(id) : this.bridge.connectAgent(id)).then(
      (agents) => this.set({ agents: listOf(agents), note: this.done(agent) }),
      (error: unknown) => this.set({ note: messageOf(error, `Could not change ${agent.name}.`) }),
    );
  }

  /** What a change did, by what the agent was before it. */
  private done(agent: LocalAgent): string {
    if (agent.connected) return `Oya removed from ${agent.name}.`;
    const asks = ALLOWED_ON_CONNECT.includes(agent.id) ? '' : ' Choose Always allow the first time it asks.';
    return `Connected. Restart ${agent.name} and it can use this browser.${asks}`;
  }
}
