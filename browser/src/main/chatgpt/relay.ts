/**
 * Answers the server's `llm_request`: a model call for a run that uses this app's
 * ChatGPT sign-in. The body is sent to OpenAI's Responses endpoint, a constant
 * here (nothing in the message can choose where it goes), with this app's token,
 * never stored and always streamed, and the raw answer goes back as a cmd_result.
 */
import type { AppServices } from '../app/services.ts';
import * as C from './constants.ts';

/** The services the relay uses. */
type Deps = Pick<AppServices, 'socket' | 'chatgpt'>;

/** The body of a model call. */
interface ModelParams {
  /** The Responses request body. */
  body?: unknown;
}

/** A model call as the server sends it. */
interface ModelRequest {
  /** The call's id, which the answer names. */
  id?: string;
  /** What to send. */
  params?: ModelParams;
}

/** OpenAI's answer, carried back whole. */
interface Relayed {
  /** Its HTTP status. */
  status: number;
  /** The whole event stream. */
  text: string;
  /** Its retry-after header, when it sent one. */
  retryAfter?: string;
}

/** The answer's outcome: the raw stream, or why there is none. */
type Outcome = Answered | Refused;

/** OpenAI answered (whatever its status). */
interface Answered {
  /** Always true. */
  ok: true;
  /** What it answered. */
  data: Relayed;
}

/** No call was made, or it never got an answer. */
interface Refused {
  /** Always false. */
  ok: false;
  /** Why. */
  error: string;
  /** A stable reason the server branches on. */
  code: string;
}

/** The request OpenAI gets: the body as the server built it, never stored, always streamed, with this app's token. */
const requestFor = (body: object, token: string): RequestInit => ({
  method: 'POST',
  headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
  body: JSON.stringify({ ...body, store: false, stream: true }),
  redirect: 'error',
  signal: AbortSignal.timeout(C.RESPONSES_TIMEOUT_MS),
});

/** One read of the stream, or a stall once nothing has come for STREAM_IDLE_MS. */
function nextChunk(reader: ReadableStreamDefaultReader<Uint8Array>) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const stalled = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(Error('The model stopped answering')), C.STREAM_IDLE_MS);
  });
  return Promise.race([reader.read(), stalled]).finally(() => clearTimeout(timer));
}

/** Whether the stream holds its final event in full: that event's line has ended, not merely begun (it carries the whole answer). */
function ended(text: string): boolean {
  const at = text.search(C.STREAM_END);
  return at >= 0 && text.indexOf('\n\n', at) >= 0;
}

/**
 * The stream's text up to its final event. Reading stops there rather than at the
 * connection's end (which OpenAI does not always send promptly), and a stream that
 * goes quiet is given up, so the server retries it instead of waiting minutes.
 */
async function readUntilDone(res: Response): Promise<string> {
  const reader = res.body!.getReader();
  const decoder = new TextDecoder();
  let text = '';
  for (let chunk = await nextChunk(reader); !chunk.done; chunk = await nextChunk(reader)) {
    text += decoder.decode(chunk.value, { stream: true });
    if (ended(text)) return (void reader.cancel().catch(() => {}), text);
  }
  return text;
}

/** Sends the body to OpenAI with `token` and reads the stream to its end. */
async function call(body: object, token: string): Promise<Outcome> {
  const res = await fetch(C.RESPONSES_URL, requestFor(body, token));
  const retryAfter = res.headers.get('retry-after') || undefined;
  const text = res.body ? await readUntilDone(res) : await res.text();
  return { ok: true, data: { status: res.status, text, retryAfter } };
}

/** The outcome for a request: refused when malformed or signed out, else OpenAI's answer. */
async function outcomeOf(deps: Deps, body: unknown): Promise<Outcome> {
  if (!body || typeof body !== 'object') return { ok: false, error: 'No model request', code: 'bad_request' };
  const token = await deps.chatgpt.accessToken().catch(() => null);
  if (!token) return { ok: false, error: 'Sign in with ChatGPT again', code: 'chatgpt_signed_out' };
  return call(body, token).catch(() => ({ ok: false, error: 'OpenAI could not be reached', code: 'unreachable' }));
}

/** Answers one `llm_request`; an answer for a socket that has since been replaced is dropped. */
export async function answerModelRequest(deps: Deps, msg: ModelRequest): Promise<void> {
  if (!msg.id) return;
  const socket = deps.socket.ws;
  const outcome = await outcomeOf(deps, msg.params?.body);
  if (deps.socket.ws !== socket) return;
  deps.socket.send({ type: 'cmd_result', id: msg.id, ...outcome });
}
