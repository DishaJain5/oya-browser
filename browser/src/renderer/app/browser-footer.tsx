/** Persistent keyboard discovery and coding-agent connections, outside the native page and workspace bounds. */
import { commandShortcut } from '../../shared/shortcuts.ts';
import { useViewModel } from '../hooks/index.ts';
import type { AgentsViewModel } from '../features/connection/index.ts';
import type { ShellRootProps } from './shell-root.tsx';
import './browser-footer.css';

/** One button per coding agent on this computer: Connect adds Oya's MCP port to it, a connected one removes it. */
function AgentButtons({ agents }: { /** The coding agents. */ agents: AgentsViewModel }) {
  const { agents: list, busy, note } = useViewModel(agents);
  if (!list.length) return null;
  return (
    <div className="footer-agents" role="group" aria-label="Coding agents">
      {note && (
        <span className="footer-note" role="status">
          {note}
        </span>
      )}
      {list.map((a) => (
        <button
          key={a.id}
          type="button"
          className="footer-agent"
          data-connected={a.connected || undefined}
          disabled={!!busy}
          title={
            a.connected
              ? `${a.name} can use this browser. Click to remove Oya from it.`
              : `Let ${a.name} use this browser through Oya's MCP port`
          }
          onClick={() => void agents.toggle(a.id)}
        >
          <span className="footer-agent-dot" aria-hidden="true" />
          {busy === a.id ? `${a.name}…` : a.connected ? a.name : `Connect ${a.name}`}
        </button>
      ))}
    </div>
  );
}

/** Both entry points are native buttons: Tab to reach, Enter or Space to open. */
export function BrowserFooter({ vms }: ShellRootProps) {
  const { dialog, palette, agents } = vms.connection;
  return (
    <footer className="browser-footer" aria-label="Browser keyboard tools">
      <button type="button" onClick={() => void dialog.open()}>
        Commands <kbd>{commandShortcut('commands', palette.platform)}</kbd>
      </button>
      <AgentButtons agents={agents} />
      <button type="button" onClick={() => void dialog.openShortcuts()}>
        Keyboard shortcuts <kbd>{commandShortcut('shortcuts', palette.platform)}</kbd>
      </button>
    </footer>
  );
}
