/**
 * Model calls for a key whose ChatGPT credential lives on the person's desktop app:
 * the request body goes to that desktop over its own control socket, the desktop
 * calls OpenAI with its token, and the raw answer comes back. The call does not go
 * through command admission, so a person holding the browser never stalls the
 * model, and nothing of the body reaches the activity log.
 */
import { HttpError } from '../../../platform/errors.ts';
import { Status } from '../../../platform/http-status.ts';
import { LlmError, type ModelTransport, type RelayedAnswer } from '../../../platform/llm/index.ts';
import { LLM_TIMEOUT_MS } from '../../../platform/constants.ts';
import { registry } from '../registry.ts';
import { describeCall } from './reporter.ts';
import { SocketTransport } from './transports/socket-transport.ts';
import type { CommandResult } from './transports/transport.ts';
import { LLM_REQUEST, SIGNED_OUT_CODE } from './constants.ts';

/** What the person is told when no desktop of theirs is connected to carry the call. */
const OFFLINE = 'Open Oya Browser on the computer where you signed in to ChatGPT, then try again.';

/** A desktop's answer as the strategy reads it; anything else is an unanswered call. */
function answerOf(result: CommandResult): RelayedAnswer {
  if (result.ok && typeof result.data?.status === 'number' && typeof result.data?.text === 'string') return result.data;
  if (result.code === SIGNED_OUT_CODE) throw new LlmError(Status.UNAUTHORIZED, 'Sign in with ChatGPT again');
  throw new LlmError(null, result.error || 'The desktop did not answer the model call');
}

/** The desktop that carries the key's model calls, or the refusal when none is connected. */
function desktopFor(apiKey: string, browserId?: string) {
  const desktop = registry.desktopOf(apiKey, browserId);
  if (!desktop) throw new HttpError(Status.UNPROCESSABLE, OFFLINE, { code: 'desktop_offline' });
  return desktop;
}

/** One body carried to `desktop` and its answer; a call that was never answered is a dropped connection. */
async function carry(desktop, body: unknown): Promise<RelayedAnswer> {
  const call = { ...describeCall(desktop.id, LLM_REQUEST, { body }, LLM_TIMEOUT_MS), visible: false };
  const result = await new SocketTransport(desktop.ws).send(call, LLM_REQUEST).catch((err) => {
    throw new LlmError(null, err?.message || 'The desktop did not answer the model call');
  });
  const answer = answerOf(result);
  console.log(`[llm] relayed responses call to ${desktop.id} → ${answer.status}`);
  return answer;
}

/** A transport that sends each model body to the key's desktop; `browserId` is the run's own browser, preferred. */
export const desktopRelay =
  (apiKey: string, browserId?: string): ModelTransport =>
  async (body) =>
    carry(desktopFor(apiKey, browserId), body);
