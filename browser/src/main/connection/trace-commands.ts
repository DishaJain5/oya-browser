/** Trace commands remain bound to an exact tab and the original agent-control revision. */
import type { CommandDeps, RemoteParams } from './commands.ts';
import type { TabHandler } from './tab-commands.ts';
/** The active page must still belong to this browser's tab manager. */
function traceTarget(deps: CommandDeps) {
  const view = deps.agentTab.view();
  if (!view || view.webContents.isDestroyed()) throw Error('No active tab');
  if (!deps.observer) throw Error('Native tracing is unavailable');
  return view.webContents;
}
/** Revoking ownership stops capture; a later grant never resumes the original trace. */
function startTrace(deps: CommandDeps) {
  const contents = traceTarget(deps),
    revision = deps.control.state.revision;
  return deps.observer.traces.start(
    contents.id,
    () => !contents.isDestroyed() && deps.control.state.mode === 'agent' && deps.control.state.revision === revision,
  );
}
/** Read the selected tab's trace; unknown and foreign ids cannot expose another page. */
function readTrace(deps: CommandDeps, params: RemoteParams = {}, stop = false) {
  return deps.observer.traces.read(traceTarget(deps).id, params, stop);
}
/** All tools pass the existing authenticated command admission path. */
export const TRACE_COMMANDS: Record<string, TabHandler> = {
  start_trace: (runner, id) => runner.sendResult(id, true, startTrace(runner.deps)),
  read_trace: (runner, id, params) => runner.sendResult(id, true, readTrace(runner.deps, params)),
  stop_trace: (runner, id, params) => runner.sendResult(id, true, readTrace(runner.deps, params, true)),
};
